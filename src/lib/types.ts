export interface FileEntry {
  name: string;
  is_dir: boolean;
  is_hidden: boolean;
  path: string;
  children?: FileEntry[];
  expanded?: boolean;
  loading?: boolean;
}
