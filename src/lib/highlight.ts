// Lightweight zero-dependency regex-based syntax highlighter

interface TokenRule {
  pattern: RegExp;
  type: string;
}

// Dracula-inspired token colors (matches github-dark-default aesthetics)
const TOKEN_COLORS: Record<string, string> = {
  keyword:    '#ff79c6',
  string:     '#f1fa8c',
  comment:    '#6272a4',
  number:     '#bd93f9',
  operator:   '#ff79c6',
  punctuation:'#f8f8f2',
  function:   '#50fa7b',
  type:       '#8be9fd',
  attribute:  '#50fa7b',
  tag:        '#ff79c6',
  property:   '#66d9ef',
  variable:   '#f8f8f2',
  regex:      '#f1fa8c',
  builtin:    '#8be9fd',
  decorator:  '#50fa7b',
  constant:   '#bd93f9',
};

const COMMON_KEYWORDS = 'break|case|catch|continue|default|do|else|finally|for|if|return|switch|throw|try|while|new|delete|typeof|instanceof|void|yield|await|async';

const LANG_RULES: Record<string, TokenRule[]> = {};

function rules(...defs: [RegExp, string][]): TokenRule[] {
  return defs.map(([pattern, type]) => ({ pattern, type }));
}

// JavaScript / TypeScript
const jsRules = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'`])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\/(?![*/])(?:\\.|\[(?:\\.|.)*?\]|.)+?\/[gimsuy]*/gm, 'regex'],
  [/\b(?:abstract|as|asserts|break|case|catch|class|const|continue|debugger|declare|default|delete|do|else|enum|export|extends|finally|for|from|function|get|if|implements|import|in|infer|instanceof|interface|is|keyof|let|module|namespace|never|new|of|package|private|protected|public|readonly|return|satisfies|set|static|super|switch|throw|try|type|typeof|undefined|unique|unknown|var|void|while|with|yield|async|await)\b/g, 'keyword'],
  [/\b(?:true|false|null|undefined|NaN|Infinity)\b/g, 'constant'],
  [/\b(?:Array|Boolean|Date|Error|Function|JSON|Map|Math|Number|Object|Promise|Proxy|RegExp|Set|String|Symbol|WeakMap|WeakSet|console|window|document|globalThis)\b/g, 'builtin'],
  [/\b(?:0[xX][\da-fA-F]+|0[oO][0-7]+|0[bB][01]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?n?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_$][\w$]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);
LANG_RULES['javascript'] = jsRules;
LANG_RULES['typescript'] = jsRules;
LANG_RULES['jsx'] = jsRules;
LANG_RULES['tsx'] = jsRules;
LANG_RULES['svelte'] = jsRules;
LANG_RULES['vue'] = jsRules;
LANG_RULES['astro'] = jsRules;

// Rust
LANG_RULES['rust'] = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/r#*"[\s\S]*?"#*/gm, 'string'],
  [/\b(?:as|async|await|break|const|continue|crate|dyn|else|enum|extern|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|ref|return|self|Self|static|struct|super|trait|type|unsafe|use|where|while|yield)\b/g, 'keyword'],
  [/\b(?:true|false)\b/g, 'constant'],
  [/\b(?:bool|char|f32|f64|i8|i16|i32|i64|i128|isize|str|u8|u16|u32|u64|u128|usize|String|Vec|Option|Result|Box|Rc|Arc|HashMap|HashSet|BTreeMap|BTreeSet)\b/g, 'type'],
  [/#\[[\w:]+/g, 'decorator'],
  [/\b(?:0[xX][\da-fA-F_]+|0[oO][0-7_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?(?:_?(?:f32|f64|i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize))?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=[(<])/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Python
LANG_RULES['python'] = rules(
  [/#.*$/gm, 'comment'],
  [/("""[\s\S]*?"""|'''[\s\S]*?''')/gm, 'string'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:and|as|assert|async|await|break|class|continue|def|del|elif|else|except|finally|for|from|global|if|import|in|is|lambda|nonlocal|not|or|pass|raise|return|try|while|with|yield)\b/g, 'keyword'],
  [/\b(?:True|False|None)\b/g, 'constant'],
  [/\b(?:int|float|str|bool|list|dict|tuple|set|bytes|type|object|range|print|len|map|filter|enumerate|zip|sorted|reversed|sum|min|max|abs|any|all|isinstance|issubclass|super|property|staticmethod|classmethod)\b/g, 'builtin'],
  [/@\w+/g, 'decorator'],
  [/\b(?:0[xX][\da-fA-F_]+|0[oO][0-7_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?j?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Go
LANG_RULES['go'] = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'`])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:break|case|chan|const|continue|default|defer|else|fallthrough|for|func|go|goto|if|import|interface|map|package|range|return|select|struct|switch|type|var)\b/g, 'keyword'],
  [/\b(?:true|false|nil|iota)\b/g, 'constant'],
  [/\b(?:bool|byte|complex64|complex128|error|float32|float64|int|int8|int16|int32|int64|rune|string|uint|uint8|uint16|uint32|uint64|uintptr|append|cap|close|complex|copy|delete|imag|len|make|new|panic|print|println|real|recover)\b/g, 'builtin'],
  [/\b(?:0[xX][\da-fA-F_]+|0[oO][0-7_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// HTML / XML / SVG
const htmlRules = rules(
  [/<!--[\s\S]*?-->/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/<\/?[\w-]+/g, 'tag'],
  [/\/?>/g, 'tag'],
  [/\b[\w-]+(?==)/g, 'attribute'],
  [/&\w+;/g, 'constant'],
);
LANG_RULES['html'] = htmlRules;
LANG_RULES['xml'] = htmlRules;
LANG_RULES['svg'] = htmlRules;

// CSS / SCSS
const cssRules = rules(
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/\/\/.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/@[\w-]+/g, 'keyword'],
  [/[.#][\w-]+/g, 'function'],
  [/\b(?:inherit|initial|unset|none|auto|transparent|currentColor|!important)\b/g, 'constant'],
  [/#(?:[\da-fA-F]{3,8})\b/g, 'number'],
  [/\b\d+(?:\.\d+)?(?:px|em|rem|%|vh|vw|vmin|vmax|ch|ex|fr|s|ms|deg|rad|turn)?\b/g, 'number'],
  [/[\w-]+(?=\s*:)/gm, 'property'],
  [/[{}()[\];,:>~+*]/g, 'punctuation'],
);
LANG_RULES['css'] = cssRules;
LANG_RULES['scss'] = cssRules;
LANG_RULES['less'] = cssRules;

// JSON
LANG_RULES['json'] = rules(
  [/(["'])(?:(?!\1|\\).|\\.)*\1\s*(?=:)/gm, 'property'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:true|false|null)\b/g, 'constant'],
  [/-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g, 'number'],
  [/[{}[\],:]/g, 'punctuation'],
);

// YAML
LANG_RULES['yaml'] = rules(
  [/#.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/^[\w][\w\s-]*(?=:)/gm, 'property'],
  [/\b(?:true|false|null|yes|no|on|off)\b/gi, 'constant'],
  [/-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g, 'number'],
  [/[[\]{},:|>-]/g, 'punctuation'],
);

// TOML
LANG_RULES['toml'] = rules(
  [/#.*$/gm, 'comment'],
  [/("""[\s\S]*?"""|'''[\s\S]*?''')/gm, 'string'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\[[\w."-]+\]/g, 'tag'],
  [/[\w-]+(?=\s*=)/gm, 'property'],
  [/\b(?:true|false)\b/g, 'constant'],
  [/-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g, 'number'],
  [/[[\]{}=,.]/g, 'punctuation'],
);

// Bash / Shell
LANG_RULES['bash'] = rules(
  [/#.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\$\{[^}]+\}|\$[\w@#?!$*-]+/g, 'variable'],
  [/\b(?:if|then|else|elif|fi|for|while|until|do|done|case|esac|in|function|return|exit|break|continue|local|export|source|alias|unalias|set|unset|shift|trap|exec|eval)\b/g, 'keyword'],
  [/\b(?:echo|printf|cd|ls|cp|mv|rm|mkdir|rmdir|cat|grep|sed|awk|find|sort|uniq|wc|head|tail|chmod|chown|curl|wget|tar|ssh|scp|git|docker|npm|node|python|pip|make|sudo)\b/g, 'builtin'],
  [/\b\d+\b/g, 'number'],
  [/[|&;()<>!{}[\]]/g, 'punctuation'],
);
LANG_RULES['sh'] = LANG_RULES['bash'];
LANG_RULES['zsh'] = LANG_RULES['bash'];
LANG_RULES['fish'] = LANG_RULES['bash'];
LANG_RULES['shell'] = LANG_RULES['bash'];

// SQL
LANG_RULES['sql'] = rules(
  [/--.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:SELECT|FROM|WHERE|AND|OR|NOT|IN|IS|NULL|AS|ON|JOIN|LEFT|RIGHT|INNER|OUTER|FULL|CROSS|INSERT|INTO|VALUES|UPDATE|SET|DELETE|CREATE|ALTER|DROP|TABLE|INDEX|VIEW|DATABASE|SCHEMA|IF|EXISTS|PRIMARY|KEY|FOREIGN|REFERENCES|UNIQUE|DEFAULT|CHECK|CONSTRAINT|ORDER|BY|GROUP|HAVING|LIMIT|OFFSET|UNION|ALL|DISTINCT|COUNT|SUM|AVG|MIN|MAX|CASE|WHEN|THEN|ELSE|END|BEGIN|COMMIT|ROLLBACK|TRANSACTION|GRANT|REVOKE|WITH|RECURSIVE)\b/gi, 'keyword'],
  [/\b(?:INT|INTEGER|BIGINT|SMALLINT|TINYINT|FLOAT|DOUBLE|DECIMAL|NUMERIC|CHAR|VARCHAR|TEXT|BLOB|DATE|TIME|DATETIME|TIMESTAMP|BOOLEAN|BOOL|SERIAL|UUID)\b/gi, 'type'],
  [/\b(?:TRUE|FALSE|NULL)\b/gi, 'constant'],
  [/-?\b\d+(?:\.\d+)?\b/g, 'number'],
  [/[()[\];,.*=<>!+\-/]/g, 'punctuation'],
);

// C / C++
const cRules = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/#\s*\w+/gm, 'keyword'],
  [/\b(?:auto|break|case|char|const|continue|default|do|double|else|enum|extern|float|for|goto|if|inline|int|long|register|restrict|return|short|signed|sizeof|static|struct|switch|typedef|union|unsigned|void|volatile|while|class|namespace|template|typename|public|private|protected|virtual|override|final|constexpr|decltype|noexcept|nullptr|static_assert|using|alignas|alignof|thread_local|try|catch|throw|new|delete|this|true|false)\b/g, 'keyword'],
  [/\b(?:true|false|NULL|nullptr|EOF)\b/g, 'constant'],
  [/\b(?:size_t|ptrdiff_t|intptr_t|uintptr_t|int8_t|int16_t|int32_t|int64_t|uint8_t|uint16_t|uint32_t|uint64_t|bool|string|vector|map|set|array|pair|tuple|shared_ptr|unique_ptr|optional|variant|any|string_view)\b/g, 'type'],
  [/\b(?:printf|scanf|malloc|free|calloc|realloc|memcpy|memset|strlen|strcmp|strcpy|strcat|fprintf|fopen|fclose|fread|fwrite|assert|exit|abort|std|cout|cin|endl|cerr|clog)\b/g, 'builtin'],
  [/\b(?:0[xX][\da-fA-F]+[uUlL]*|0[bB][01]+[uUlL]*|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFlLuU]*)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~#]/g, 'punctuation'],
);
LANG_RULES['c'] = cRules;
LANG_RULES['cpp'] = cRules;

// Markdown
LANG_RULES['markdown'] = rules(
  [/^#{1,6}\s+.+$/gm, 'keyword'],
  [/\*\*[^*]+\*\*/g, 'keyword'],
  [/\*[^*]+\*/g, 'string'],
  [/`[^`]+`/g, 'function'],
  [/\[([^\]]+)\]\([^)]+\)/g, 'type'],
  [/^[-*+]\s/gm, 'punctuation'],
  [/^\d+\.\s/gm, 'punctuation'],
  [/^>/gm, 'comment'],
  [/^---$/gm, 'punctuation'],
);
LANG_RULES['mdx'] = LANG_RULES['markdown'];

// GraphQL
LANG_RULES['graphql'] = rules(
  [/#.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:query|mutation|subscription|fragment|on|type|interface|union|enum|scalar|input|extend|implements|directive|schema)\b/g, 'keyword'],
  [/\b(?:Int|Float|String|Boolean|ID)\b/g, 'type'],
  [/\b(?:true|false|null)\b/g, 'constant'],
  [/-?\b\d+(?:\.\d+)?\b/g, 'number'],
  [/[{}()[\]:!=@$&|]/g, 'punctuation'],
);
LANG_RULES['gql'] = LANG_RULES['graphql'];

// Dockerfile
LANG_RULES['dockerfile'] = rules(
  [/#.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:FROM|RUN|CMD|LABEL|MAINTAINER|EXPOSE|ENV|ADD|COPY|ENTRYPOINT|VOLUME|USER|WORKDIR|ARG|ONBUILD|STOPSIGNAL|HEALTHCHECK|SHELL|AS)\b/g, 'keyword'],
  [/\$\{[^}]+\}|\$[\w]+/g, 'variable'],
  [/\b\d+\b/g, 'number'],
);

// INI / .env
LANG_RULES['ini'] = rules(
  [/[#;].*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\[[\w.-]+\]/g, 'tag'],
  [/^[\w.-]+(?=\s*=)/gm, 'property'],
  [/\b(?:true|false|yes|no|on|off)\b/gi, 'constant'],
  [/-?\b\d+(?:\.\d+)?\b/g, 'number'],
  [/[=]/g, 'punctuation'],
);

// PHP
LANG_RULES['php'] = rules(
  [/\/\/.*$|#.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\$\w+/g, 'variable'],
  [/\b(?:abstract|and|array|as|break|callable|case|catch|class|clone|const|continue|declare|default|die|do|echo|else|elseif|empty|enddeclare|endfor|endforeach|endif|endswitch|endwhile|eval|exit|extends|final|finally|fn|for|foreach|function|global|goto|if|implements|include|include_once|instanceof|insteadof|interface|isset|list|match|namespace|new|or|print|private|protected|public|readonly|require|require_once|return|static|switch|throw|trait|try|unset|use|var|while|xor|yield)\b/g, 'keyword'],
  [/\b(?:true|false|null|TRUE|FALSE|NULL)\b/g, 'constant'],
  [/\b(?:0[xX][\da-fA-F]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Ruby
LANG_RULES['ruby'] = rules(
  [/#.*$/gm, 'comment'],
  [/=begin[\s\S]*?=end/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/:\w+/g, 'constant'],
  [/\b(?:alias|and|begin|break|case|class|def|defined\?|do|else|elsif|end|ensure|for|if|in|module|next|nil|not|or|redo|rescue|retry|return|self|super|then|undef|unless|until|when|while|yield|require|include|extend|attr_accessor|attr_reader|attr_writer|puts|print|raise|lambda|proc)\b/g, 'keyword'],
  [/\b(?:true|false|nil|self|__FILE__|__LINE__|__dir__)\b/g, 'constant'],
  [/@{1,2}\w+/g, 'variable'],
  [/\$\w+/g, 'variable'],
  [/\b\d+(?:\.\d+)?\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*[!?]?)\s*(?=[\s(])/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Swift
LANG_RULES['swift'] = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:associatedtype|break|case|catch|class|continue|convenience|default|defer|deinit|do|else|enum|extension|fallthrough|fileprivate|final|for|func|guard|if|import|in|indirect|infix|init|inout|internal|is|lazy|let|mutating|nonmutating|open|operator|optional|override|postfix|precedencegroup|prefix|private|protocol|public|repeat|required|rethrows|return|self|Self|some|static|struct|subscript|super|switch|throw|throws|try|typealias|unowned|var|weak|where|while|async|await)\b/g, 'keyword'],
  [/\b(?:true|false|nil)\b/g, 'constant'],
  [/\b(?:Any|AnyObject|Array|Bool|Character|Dictionary|Double|Float|Int|Optional|Set|String|Void|Error)\b/g, 'type'],
  [/\b(?:0[xX][\da-fA-F_]+|0[oO][0-7_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Kotlin
LANG_RULES['kotlin'] = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/("""[\s\S]*?"""|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')/gm, 'string'],
  [/\b(?:abstract|actual|annotation|as|break|by|catch|class|companion|const|constructor|continue|crossinline|data|do|else|enum|expect|external|final|finally|for|fun|get|if|import|in|infix|init|inline|inner|interface|internal|is|lateinit|noinline|object|open|operator|out|override|package|private|protected|public|reified|return|sealed|set|super|suspend|tailrec|this|throw|try|typealias|val|var|vararg|when|where|while|yield)\b/g, 'keyword'],
  [/\b(?:true|false|null)\b/g, 'constant'],
  [/\b(?:Any|Boolean|Byte|Char|Double|Float|Int|Long|Nothing|Short|String|Unit|Array|List|Map|Set|Pair|Triple)\b/g, 'type'],
  [/\b(?:0[xX][\da-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?[fFL]?)\b/g, 'number'],
  [/@\w+/g, 'decorator'],
  [/\$\{[^}]+\}|\$\w+/g, 'variable'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Java
LANG_RULES['java'] = rules(
  [/\/\/.*$/gm, 'comment'],
  [/\/\*[\s\S]*?\*\//gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:abstract|assert|break|case|catch|class|const|continue|default|do|else|enum|extends|final|finally|for|goto|if|implements|import|instanceof|interface|native|new|package|private|protected|public|return|static|strictfp|super|switch|synchronized|this|throw|throws|transient|try|var|void|volatile|while|yield|record|sealed|permits|non-sealed)\b/g, 'keyword'],
  [/\b(?:true|false|null)\b/g, 'constant'],
  [/\b(?:boolean|byte|char|double|float|int|long|short|String|Object|Integer|Long|Double|Float|Boolean|Character|Byte|Short|Void|Class|System|Math|Arrays|Collections|List|ArrayList|Map|HashMap|Set|HashSet|Optional|Stream|Thread|Runnable)\b/g, 'type'],
  [/@\w+/g, 'decorator'],
  [/\b(?:0[xX][\da-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?[fFdDlL]?)\b/g, 'number'],
  [/(?<=[\s,([{=:!&|?+\-*/^~]|^)([a-zA-Z_][\w]*)\s*(?=\()/gm, 'function'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~@#]/g, 'punctuation'],
);

// Lua
LANG_RULES['lua'] = rules(
  [/--\[\[[\s\S]*?\]\]|--.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1|\[\[[\s\S]*?\]\]/gm, 'string'],
  [/\b(?:and|break|do|else|elseif|end|for|function|goto|if|in|local|not|or|repeat|return|then|until|while)\b/g, 'keyword'],
  [/\b(?:true|false|nil)\b/g, 'constant'],
  [/\b(?:print|type|tostring|tonumber|error|pcall|xpcall|assert|require|pairs|ipairs|next|select|unpack|rawget|rawset|setmetatable|getmetatable|table|string|math|io|os|coroutine)\b/g, 'builtin'],
  [/\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g, 'number'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~#]/g, 'punctuation'],
);

// Zig
LANG_RULES['zig'] = rules(
  [/\/\/.*$/gm, 'comment'],
  [/(["'])(?:(?!\1|\\).|\\.)*\1/gm, 'string'],
  [/\b(?:addrspace|align|allowzero|and|anyframe|anytype|asm|async|await|break|callconv|catch|comptime|const|continue|defer|else|enum|errdefer|error|export|extern|fn|for|if|inline|linksection|noalias|nosuspend|orelse|or|packed|pub|resume|return|struct|suspend|switch|test|threadlocal|try|union|unreachable|var|volatile|while)\b/g, 'keyword'],
  [/\b(?:true|false|null|undefined)\b/g, 'constant'],
  [/\b(?:bool|c_int|c_long|c_char|comptime_int|comptime_float|f16|f32|f64|f128|i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize|void|anyerror|type)\b/g, 'type'],
  [/\b(?:0[xX][\da-fA-F_]+|0[oO][0-7_]+|0[bB][01_]+|\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?\d+)?)\b/g, 'number'],
  [/@\w+/g, 'builtin'],
  [/[{}()[\];,.:?!&|=<>+\-*/%^~#]/g, 'punctuation'],
);

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

interface Token {
  start: number;
  end: number;
  type: string;
}

function tokenize(code: string, langRules: TokenRule[]): Token[] {
  const tokens: Token[] = [];
  for (const rule of langRules) {
    const re = new RegExp(rule.pattern.source, rule.pattern.flags);
    let m: RegExpExecArray | null;
    while ((m = re.exec(code)) !== null) {
      tokens.push({ start: m.index, end: m.index + m[0].length, type: rule.type });
      if (!re.global) break;
    }
  }
  // Sort by start, prefer longer matches at same position
  tokens.sort((a, b) => a.start - b.start || (b.end - b.start) - (a.end - a.start));
  // Remove overlapping tokens (first match wins at each position)
  const filtered: Token[] = [];
  let lastEnd = 0;
  for (const t of tokens) {
    if (t.start >= lastEnd) {
      filtered.push(t);
      lastEnd = t.end;
    }
  }
  return filtered;
}

export function highlightToHtml(code: string, language: string): string {
  const lang = language.toLowerCase();
  const langRules = LANG_RULES[lang];

  if (!langRules) {
    return `<pre class="hl-pre"><code class="hl-code">${escapeHtml(code)}</code></pre>`;
  }

  const tokens = tokenize(code, langRules);
  const parts: string[] = [];
  let pos = 0;

  for (const t of tokens) {
    if (t.start > pos) {
      parts.push(escapeHtml(code.slice(pos, t.start)));
    }
    const color = TOKEN_COLORS[t.type] || '';
    if (color) {
      parts.push(`<span style="color:${color}">${escapeHtml(code.slice(t.start, t.end))}</span>`);
    } else {
      parts.push(escapeHtml(code.slice(t.start, t.end)));
    }
    pos = t.end;
  }
  if (pos < code.length) {
    parts.push(escapeHtml(code.slice(pos)));
  }

  return `<pre class="hl-pre"><code class="hl-code">${parts.join('')}</code></pre>`;
}

export function highlightLines(code: string, language: string): string {
  const lang = language.toLowerCase();
  const langRules = LANG_RULES[lang];

  if (!langRules) {
    const lines = code.split('\n');
    return lines.map(line =>
      `<span class="line">${escapeHtml(line) || ' '}</span>`
    ).join('\n');
  }

  const tokens = tokenize(code, langRules);
  const parts: string[] = [];
  let pos = 0;

  for (const t of tokens) {
    if (t.start > pos) {
      parts.push(escapeHtml(code.slice(pos, t.start)));
    }
    const color = TOKEN_COLORS[t.type] || '';
    if (color) {
      parts.push(`<span style="color:${color}">${escapeHtml(code.slice(t.start, t.end))}</span>`);
    } else {
      parts.push(escapeHtml(code.slice(t.start, t.end)));
    }
    pos = t.end;
  }
  if (pos < code.length) {
    parts.push(escapeHtml(code.slice(pos)));
  }

  // Wrap each line in a <span class="line">
  const html = parts.join('');
  const lines = html.split('\n');
  return lines.map(line => `<span class="line">${line || ' '}</span>`).join('\n');
}
