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

export interface ChartAxis {
  readonly bounds: readonly [number, number];
  readonly title?: string;
  readonly labels?: readonly TextLine[];
  readonly style?: Style;
}

export interface ChartDataset {
  readonly name?: string;
  readonly data: readonly (readonly [number, number])[];
  readonly graphType?: "line" | "scatter" | "bar";
  readonly style?: Style;
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
  }
  | {
    readonly type: "table";
    readonly id: string;
    readonly rows: readonly (readonly TextLine[])[];
    readonly widths: readonly Constraint[];
    readonly header?: readonly TextLine[];
    readonly selected?: number;
    readonly offset?: number;
    readonly columnSpacing?: number;
    readonly style?: Style;
    readonly highlightStyle?: Style;
  }
  | {
    readonly type: "tabs";
    readonly id: string;
    readonly titles: readonly TextLine[];
    readonly selected?: number;
    readonly style?: Style;
    readonly highlightStyle?: Style;
  }
  | {
    readonly type: "gauge";
    readonly ratio: number;
    readonly label?: TextSpan;
    readonly style?: Style;
    readonly gaugeStyle?: Style;
  }
  | {
    readonly type: "chart";
    readonly datasets: readonly ChartDataset[];
    readonly xAxis: ChartAxis;
    readonly yAxis: ChartAxis;
    readonly style?: Style;
  }
  | {
    readonly type: "scrollbar";
    readonly id: string;
    readonly contentLength: number;
    readonly position?: number;
    readonly viewportContentLength?: number;
    readonly orientation?:
      | "verticalRight"
      | "verticalLeft"
      | "horizontalBottom"
      | "horizontalTop";
    readonly style?: Style;
  };

export interface FrameDescription {
  readonly protocolVersion: typeof PROTOCOL_VERSION;
  readonly root: UiNode;
}

export interface WidgetStateUpdate {
  readonly id: string;
  /** List/table scroll offset, scrollbar position, or zero for tabs. */
  readonly offset: number;
  /** List item, table data row, or tab index; omitted when unselected. */
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
  | "io"
  | "terminalBusy"
  | "terminalPoisoned"
  | "notTerminal"
  | "rawModeActive"
  | "closed"
  | "renderingFailed"
  | "concurrentEventWait"
  | "invalidTimeout"
  | "input"
  | "panic"
  | "initialization"
  | "shutdown"
  | "invalidArgument"
  | "unsupportedAbi"
  | "unsupportedCapability";

export interface ErrorDescription {
  readonly code: ErrorCode;
  readonly message: string;
  readonly path?: string;
  readonly operation?: string;
  readonly cause?: ErrorDescription;
  readonly cleanup?: readonly CleanupFailure[];
}

export interface CleanupFailure {
  readonly operation: string;
  readonly message: string;
}

export type KeyModifier =
  | "shift"
  | "control"
  | "alt"
  | "super"
  | "hyper"
  | "meta";
export type KeyState = "keypad" | "capsLock" | "numLock";
export type MediaKeyCode =
  | "play"
  | "pause"
  | "playPause"
  | "reverse"
  | "stop"
  | "fastForward"
  | "rewind"
  | "trackNext"
  | "trackPrevious"
  | "record"
  | "lowerVolume"
  | "raiseVolume"
  | "muteVolume";
export type ModifierKeyCode =
  | "leftShift"
  | "leftControl"
  | "leftAlt"
  | "leftSuper"
  | "leftHyper"
  | "leftMeta"
  | "rightShift"
  | "rightControl"
  | "rightAlt"
  | "rightSuper"
  | "rightHyper"
  | "rightMeta"
  | "isoLevel3Shift"
  | "isoLevel5Shift";
export type KeyCode =
  | { readonly type: "character"; readonly value: string }
  | { readonly type: "function"; readonly value: number }
  | { readonly type: "media"; readonly value: MediaKeyCode }
  | { readonly type: "modifier"; readonly value: ModifierKeyCode }
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
      | "delete"
      | "null"
      | "capsLock"
      | "scrollLock"
      | "numLock"
      | "printScreen"
      | "pause"
      | "menu"
      | "keypadBegin";
  };
export type MouseButton = "left" | "right" | "middle";
export type MouseKind =
  | { readonly type: "down" | "up" | "drag"; readonly button: MouseButton }
  | {
    readonly type:
      | "moved"
      | "scrollUp"
      | "scrollDown"
      | "scrollLeft"
      | "scrollRight";
  };
export type TerminalEvent =
  | {
    readonly type: "key";
    readonly key: KeyCode;
    readonly kind: "press" | "repeat" | "release";
    readonly modifiers: readonly KeyModifier[];
    /** Omitted when no keyboard state was reported. */
    readonly state?: readonly KeyState[];
  }
  | {
    readonly type: "mouse";
    readonly kind: MouseKind;
    /** Zero-based terminal cell coordinates. */
    readonly column: number;
    readonly row: number;
    readonly modifiers: readonly KeyModifier[];
  }
  | { readonly type: "paste"; readonly text: string }
  | { readonly type: "focus"; readonly focused: boolean }
  | {
    readonly type: "resize";
    readonly width: number;
    readonly height: number;
  };
