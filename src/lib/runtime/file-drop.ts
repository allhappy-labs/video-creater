/**
 * Runtime-neutral external file drop events. `over` marks the drop target active (it may repeat
 * while the pointer moves), `leave` clears it, and `drop` clears it and carries the dropped paths.
 */
export type FileDropEvent =
  | { readonly type: "over" }
  | { readonly type: "leave" }
  | {
      readonly type: "drop";
      readonly paths: readonly string[];
      readonly files?: readonly File[];
    };

export type FileDropHandler = (event: FileDropEvent) => void;

export type StopFileDrops = () => void;
