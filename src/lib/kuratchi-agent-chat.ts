import { AgentClient } from 'agents/client';
import type { UIMessage } from 'ai';
import { MessageType } from '@cloudflare/ai-chat/types';
import type { IncomingMessage, OutgoingMessage } from '@cloudflare/ai-chat/types';

export type KuratchiChatStatus = 'idle' | 'submitted' | 'streaming' | 'error';

type KuratchiAgentChatOptions<Message extends UIMessage = UIMessage> = {
  onMessagesChange?: (messages: Message[]) => void;
  onStatusChange?: (status: KuratchiChatStatus) => void;
  onError?: (error: Error) => void;
};

function findLastPartByType(parts: any[], type: string) {
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    if (parts[index]?.type === type) return parts[index];
  }
  return null;
}

function findToolPartByCallId(parts: any[], toolCallId: string) {
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    if (parts[index]?.toolCallId === toolCallId) return parts[index];
  }
  return null;
}

function applyChunkToParts(parts: any[], chunk: any): boolean {
  switch (chunk?.type) {
    case 'text-start':
      parts.push({ type: 'text', text: '', state: 'streaming' });
      return true;
    case 'text-delta': {
      const lastTextPart = findLastPartByType(parts, 'text');
      if (lastTextPart?.type === 'text') {
        lastTextPart.text += chunk.delta ?? '';
      } else {
        parts.push({ type: 'text', text: chunk.delta ?? '', state: 'streaming' });
      }
      return true;
    }
    case 'text-end': {
      const lastTextPart = findLastPartByType(parts, 'text');
      if (lastTextPart && 'state' in lastTextPart) lastTextPart.state = 'done';
      return true;
    }
    case 'reasoning-start':
      parts.push({ type: 'reasoning', text: '', state: 'streaming' });
      return true;
    case 'reasoning-delta': {
      const lastReasoningPart = findLastPartByType(parts, 'reasoning');
      if (lastReasoningPart?.type === 'reasoning') {
        lastReasoningPart.text += chunk.delta ?? '';
      } else {
        parts.push({ type: 'reasoning', text: chunk.delta ?? '', state: 'streaming' });
      }
      return true;
    }
    case 'reasoning-end': {
      const lastReasoningPart = findLastPartByType(parts, 'reasoning');
      if (lastReasoningPart && 'state' in lastReasoningPart) lastReasoningPart.state = 'done';
      return true;
    }
    case 'tool-input-start':
      parts.push({
        type: `tool-${chunk.toolName}`,
        toolCallId: chunk.toolCallId,
        toolName: chunk.toolName,
        state: 'input-streaming',
        input: undefined,
      });
      return true;
    case 'tool-input-delta': {
      const toolPart = findToolPartByCallId(parts, chunk.toolCallId);
      if (toolPart) toolPart.input = chunk.input;
      return true;
    }
    case 'tool-input-available': {
      const existing = findToolPartByCallId(parts, chunk.toolCallId);
      if (existing) {
        existing.state = 'input-available';
        existing.input = chunk.input;
      } else {
        parts.push({
          type: `tool-${chunk.toolName}`,
          toolCallId: chunk.toolCallId,
          toolName: chunk.toolName,
          state: 'input-available',
          input: chunk.input,
        });
      }
      return true;
    }
    case 'tool-output-available': {
      const existing = findToolPartByCallId(parts, chunk.toolCallId);
      if (existing) {
        existing.state = 'output-available';
        existing.output = chunk.output;
      }
      return true;
    }
    case 'tool-output-error': {
      const existing = findToolPartByCallId(parts, chunk.toolCallId);
      if (existing) {
        existing.state = 'output-error';
        existing.errorText = chunk.errorText;
      }
      return true;
    }
    case 'tool-input-error': {
      const existing = findToolPartByCallId(parts, chunk.toolCallId);
      if (existing) {
        existing.state = 'input-error';
        existing.errorText = chunk.errorText;
      }
      return true;
    }
    default:
      return false;
  }
}

function mergeAssistantMessage<Message extends UIMessage>(
  messages: Message[],
  nextMessage: Message,
  continuation: boolean
): Message[] {
  const nextMessages = [...messages];
  if (continuation) {
    const lastAssistantIndex = [...nextMessages].findLastIndex((message) => message.role === 'assistant');
    if (lastAssistantIndex >= 0) {
      nextMessages[lastAssistantIndex] = nextMessage;
    } else {
      nextMessages.push(nextMessage);
    }
    return nextMessages;
  }

  const existingIndex = nextMessages.findIndex((message) => message.id === nextMessage.id);
  if (existingIndex >= 0) {
    nextMessages[existingIndex] = nextMessage;
  } else {
    nextMessages.push(nextMessage);
  }
  return nextMessages;
}

export class KuratchiAgentChat<Message extends UIMessage = UIMessage> {
  readonly client: AgentClient<unknown>;
  private readonly options: KuratchiAgentChatOptions<Message>;
  private readonly messageListener: (event: MessageEvent) => void;
  private messages: Message[] = [];
  private status: KuratchiChatStatus = 'idle';
  private activeRequestId = '';
  private activeRequestIds = new Set<string>();
  private activeStream: { id: string; messageId: string; parts: unknown[]; metadata?: Record<string, unknown> } | null = null;

  constructor(client: AgentClient<unknown>, options: KuratchiAgentChatOptions<Message> = {}) {
    this.client = client;
    this.options = options;
    this.messageListener = (event: MessageEvent) => {
      this.handleSocketMessage(event);
    };
  }

  get currentMessages(): Message[] {
    return this.messages;
  }

  get currentStatus(): KuratchiChatStatus {
    return this.status;
  }

  hasActiveRequest(): boolean {
    return this.activeRequestIds.size > 0;
  }

  hydrate(messages: Message[]) {
    this.messages = [...messages];
    this.emitMessages();
  }

  attach() {
    this.client.addEventListener('message', this.messageListener);
  }

  detach() {
    this.client.removeEventListener('message', this.messageListener);
  }

  close() {
    this.detach();
    this.activeStream = null;
    this.client.close();
  }

  resume() {
    this.send({
      type: MessageType.CF_AGENT_STREAM_RESUME_REQUEST,
    });
  }

  sendMessage(message: Message, body: Record<string, unknown> = {}) {
    const requestId = crypto.randomUUID();
    this.activeRequestId = requestId;
    this.activeRequestIds.add(requestId);
    this.messages = [...this.messages, message];
    this.emitMessages();
    this.setStatus('submitted');
    this.send({
      type: MessageType.CF_AGENT_USE_CHAT_REQUEST,
      id: requestId,
      init: {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          messages: this.messages,
          ...body,
        }),
      },
    });
    return requestId;
  }

  cancelActiveRequest() {
    if (!this.activeRequestIds.size) return;

    for (const requestId of [...this.activeRequestIds]) {
      this.send({
        type: MessageType.CF_AGENT_CHAT_REQUEST_CANCEL,
        id: requestId,
      } as IncomingMessage<Message>);
    }

    this.activeRequestIds.clear();
    this.activeRequestId = '';
    this.activeStream = null;
    this.setStatus('idle');
  }

  addToolApprovalResponse(toolCallId: string, approved: boolean, autoContinue = true) {
    this.send({
      type: MessageType.CF_AGENT_TOOL_APPROVAL,
      toolCallId,
      approved,
      autoContinue,
    });
  }

  addToolOutput(options: {
    toolCallId: string;
    toolName: string;
    output: unknown;
    state?: 'output-available' | 'output-error';
    errorText?: string;
    autoContinue?: boolean;
  }) {
    this.send({
      type: MessageType.CF_AGENT_TOOL_RESULT,
      toolCallId: options.toolCallId,
      toolName: options.toolName,
      output: options.output,
      state: options.state,
      errorText: options.errorText,
      autoContinue: options.autoContinue,
    });
  }

  clearHistory() {
    this.messages = [];
    this.emitMessages();
    this.send({
      type: MessageType.CF_AGENT_CHAT_CLEAR,
    });
  }

  private emitMessages() {
    this.options.onMessagesChange?.([...this.messages]);
  }

  private setStatus(status: KuratchiChatStatus) {
    this.status = status;
    this.options.onStatusChange?.(status);
  }

  private reportError(error: unknown) {
    this.setStatus('error');
    this.options.onError?.(error instanceof Error ? error : new Error(String(error)));
  }

  private send(message: IncomingMessage<Message>) {
    this.client.send(JSON.stringify(message));
  }

  private flushActiveStreamToMessages(activeStream: { messageId: string; parts: unknown[]; metadata?: Record<string, unknown> }) {
    const nextMessage = {
      id: activeStream.messageId as Message['id'],
      role: 'assistant',
      parts: [...activeStream.parts],
      ...(activeStream.metadata ? { metadata: activeStream.metadata } : {}),
    } as Message;

    this.messages = mergeAssistantMessage(this.messages, nextMessage, false);
    this.emitMessages();
  }

  private ensureActiveStream(requestId: string, continuation: boolean) {
    if (this.activeStream && this.activeStream.id === requestId) {
      return this.activeStream;
    }

    let messageId: Message['id'] = crypto.randomUUID() as Message['id'];
    let parts: unknown[] = [];
    let metadata: Record<string, unknown> | undefined;

    if (continuation) {
      const lastAssistant = [...this.messages].findLast((message) => message.role === 'assistant') as Message | undefined;
      if (lastAssistant) {
        messageId = lastAssistant.id as Message['id'];
        parts = Array.isArray(lastAssistant.parts) ? [...lastAssistant.parts] : [];
        metadata = (lastAssistant as Message & { metadata?: Record<string, unknown> }).metadata
          ? { ...(lastAssistant as Message & { metadata?: Record<string, unknown> }).metadata }
          : undefined;
      }
    }

    this.activeStream = { id: requestId, messageId, parts, metadata };
    return this.activeStream;
  }

  private handleSocketMessage(event: MessageEvent) {
    if (typeof event.data !== 'string') return;

    let parsed: OutgoingMessage<Message> | null = null;
    try {
      parsed = JSON.parse(event.data) as OutgoingMessage<Message>;
    } catch {
      return;
    }
    if (!parsed) return;

    if (parsed.type === MessageType.CF_AGENT_CHAT_MESSAGES) {
      this.messages = parsed.messages;
      this.emitMessages();
      if (!this.activeRequestIds.size) {
        this.setStatus('idle');
      }
      return;
    }

    if (parsed.type === MessageType.CF_AGENT_MESSAGE_UPDATED) {
      this.messages = mergeAssistantMessage(this.messages, parsed.message, true);
      this.emitMessages();
      return;
    }

    if (parsed.type === MessageType.CF_AGENT_STREAM_RESUMING) {
      this.send({
        type: MessageType.CF_AGENT_STREAM_RESUME_ACK,
        id: parsed.id,
      });
      return;
    }

    if (parsed.type === MessageType.CF_AGENT_STREAM_RESUME_NONE) {
      if (!this.activeRequestIds.size && (this.status === 'submitted' || this.status === 'streaming')) {
        this.setStatus('idle');
      }
      return;
    }

    if (parsed.type !== MessageType.CF_AGENT_USE_CHAT_RESPONSE) return;

    if (parsed.error) {
      this.activeRequestIds.delete(parsed.id);
      if (!this.activeRequestIds.size) {
        this.activeRequestId = '';
      }
      this.reportError(new Error('Kuratchi live agent stream failed'));
      return;
    }

    if (parsed.id !== this.activeRequestId || !parsed.body) {
      if (parsed.done) {
        this.activeRequestIds.delete(parsed.id);
        if (!this.activeRequestIds.size) {
          this.activeRequestId = '';
          this.setStatus('idle');
        }
      }
      return;
    }

    try {
      const activeStream = this.ensureActiveStream(parsed.id, parsed.continuation === true);
      const chunkData = JSON.parse(parsed.body);
      const handled = applyChunkToParts(activeStream.parts, chunkData);

      if (!handled && typeof chunkData?.type === 'string') {
        if (chunkData.type === 'start' && typeof chunkData.messageId === 'string') {
          activeStream.messageId = chunkData.messageId as string;
        } else if (chunkData.type === 'message-metadata' && chunkData.messageMetadata) {
          activeStream.metadata = activeStream.metadata
            ? { ...activeStream.metadata, ...chunkData.messageMetadata }
            : { ...chunkData.messageMetadata };
        }
      }

      if (!parsed.replay) {
        this.flushActiveStreamToMessages(activeStream);
      }
      this.setStatus('streaming');
    } catch (error) {
      this.reportError(error);
      return;
    }

    if (parsed.done) {
      this.activeRequestIds.delete(parsed.id);
      if (!this.activeRequestIds.size) {
        this.activeRequestId = '';
      }
      if (parsed.replay && this.activeStream) {
        this.flushActiveStreamToMessages(this.activeStream);
      }
      this.activeStream = null;
      if (!this.activeRequestIds.size) {
        this.setStatus('idle');
      }
    } else if (parsed.replayComplete && this.activeStream) {
      this.flushActiveStreamToMessages(this.activeStream);
    }
  }
}
