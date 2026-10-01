/** Runtime-independent application contracts. Platform adapters are injected. */
import {
  type FrameDescription,
  PROTOCOL_VERSION,
  type RenderResult,
  type TerminalEvent,
  type UiNode,
  validateFrame,
} from "@ratatui-js/protocol";

export type {
  Color,
  Constraint,
  FrameDescription,
  KeyCode,
  KeyModifier,
  KeyState,
  LayoutChild,
  MediaKeyCode,
  ModifierKeyCode,
  MouseButton,
  MouseKind,
  Padding,
  RenderResult,
  Style,
  TerminalEvent,
  TextLine,
  TextSpan,
  UiNode,
} from "@ratatui-js/protocol";

export interface TerminalOptions {
  /** Defaults to true for real-terminal adapters. */
  readonly alternateScreen?: boolean;
  /** Opt-in modes default to false and are disabled on close. */
  readonly mouseCapture?: boolean;
  readonly bracketedPaste?: boolean;
  readonly focusReporting?: boolean;
  /** Require terminal support; unsupported terminals reject open(). */
  readonly enhancedKeyboard?: boolean;
}

export interface TerminalCapabilities {
  readonly protocolVersion: typeof PROTOCOL_VERSION;
  readonly keyboard: boolean;
  readonly resize: boolean;
  /** True when the adapter enabled the requested mode. Delivery also depends on the terminal. */
  readonly mouse?: boolean;
  readonly paste?: boolean;
  readonly focus?: boolean;
  readonly enhancedKeyboard?: boolean;
}

/** A platform adapter owns native loading, memory, and concurrency safety. */
export interface PlatformAdapter {
  open(options: TerminalOptions): Promise<TerminalDriver>;
}

export interface TerminalDriver {
  readonly capabilities: TerminalCapabilities;
  /** Resolves after native drawing/output completes, not after queue submission. */
  render(frame: FrameDescription): Promise<RenderResult>;
  /** One outstanding wait is allowed. Returns null once closed. */
  nextEvent(): Promise<TerminalEvent | null>;
  /** Idempotent; wakes event waits and waits for safe native shutdown. */
  close(): Promise<void>;
}

/** Validate and wrap a UI description for submission to a platform driver. */
export function createFrame(root: UiNode): FrameDescription {
  const frame: FrameDescription = { protocolVersion: PROTOCOL_VERSION, root };
  validateFrame(frame);
  return frame;
}
export { block, column, list, paragraph, row } from "./builders.ts";
export {
  type Application,
  type ApplicationDefinition,
  EXIT,
  runApplication,
  startApplication,
} from "./application.ts";
