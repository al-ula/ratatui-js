/** Version of the JSON rendering protocol, independent of the native ABI. */
export const PROTOCOL_VERSION = 1 as const;

/** Shared limits enforced before a frame is rendered. */
export const FRAME_LIMITS = {
  bytes: 1_048_576,
  depth: 32,
  nodes: 4_096,
  collectionItems: 4_096,
  spans: 16_384,
} as const;

export type Color =
  | "black"
  | "red"
  | "green"
  | "yellow"
  | "blue"
  | "magenta"
  | "cyan"
  | "gray"
  | "darkGray"
  | "lightRed"
  | "lightGreen"
  | "lightYellow"
  | "lightBlue"
  | "lightMagenta"
  | "lightCyan"
  | "white";

export interface Style {
  readonly fg?: Color;
  readonly bg?: Color;
  readonly bold?: boolean;
  readonly dim?: boolean;
  readonly italic?: boolean;
  readonly underlined?: boolean;
  readonly reversed?: boolean;
  readonly crossedOut?: boolean;
}

export interface TextSpan {
  readonly text: string;
  readonly style?: Style;
}

/** An explicit line; spans must not contain newlines or control characters. */
export type TextLine = readonly TextSpan[];

export type Constraint =
  | { readonly kind: "length" | "min" | "max"; readonly value: number }
  | { readonly kind: "percentage"; readonly value: number }
  | { readonly kind: "fill"; readonly value: number };

export interface LayoutChild {
  readonly constraint: Constraint;
  readonly node: UiNode;
}

export interface Padding {
  readonly left?: number;
  readonly right?: number;
  readonly top?: number;
  readonly bottom?: number;
}

export type UiNode =
  | {
    readonly type: "row" | "column";
    readonly children: readonly LayoutChild[];
    readonly spacing?: number;
  }
  | {
    readonly type: "block";
    readonly child: UiNode;
    readonly title?: string;
    readonly border?: "plain" | "rounded" | "double" | "none";
    readonly padding?: Padding;
    readonly style?: Style;
  }
  | {
    readonly type: "paragraph";
    readonly lines: readonly TextLine[];
    readonly wrap?: boolean;
    readonly style?: Style;
  }
  | {
    readonly type: "list";
    readonly id: string;
    readonly items: readonly TextLine[];
    readonly selected?: number;
    readonly offset?: number;
    readonly style?: Style;
    readonly highlightStyle?: Style;
  };

export interface FrameDescription {
  readonly protocolVersion: typeof PROTOCOL_VERSION;
  readonly root: UiNode;
}

export interface WidgetStateUpdate {
  readonly id: string;
  readonly offset: number;
  readonly selected?: number;
}

export interface RenderResult {
  readonly width: number;
  readonly height: number;
  readonly widgetStates: readonly WidgetStateUpdate[];
}

export type ErrorCode =
  | "invalidJson"
  | "invalidFrame"
  | "unsupportedProtocol"
  | "io";

export interface ErrorDescription {
  readonly code: ErrorCode;
  readonly message: string;
  readonly path?: string;
}

export type KeyCode =
  | { readonly type: "character"; readonly value: string }
  | { readonly type: "function"; readonly value: number }
  | {
    readonly type:
      | "enter"
      | "escape"
      | "backspace"
      | "tab"
      | "backTab"
      | "left"
      | "right"
      | "up"
      | "down"
      | "home"
      | "end"
      | "pageUp"
      | "pageDown"
      | "insert"
      | "delete";
  };

export type TerminalEvent =
  | {
    readonly type: "key";
    readonly key: KeyCode;
    readonly kind: "press" | "repeat" | "release";
    readonly modifiers: readonly ("shift" | "control" | "alt" | "super")[];
  }
  | {
    readonly type: "resize";
    readonly width: number;
    readonly height: number;
  };
