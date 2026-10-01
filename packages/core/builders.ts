import type { LayoutChild, TextLine, UiNode } from "@ratatui-js/protocol";
type NodeOf<Type extends UiNode["type"]> = Extract<
  UiNode,
  { readonly type: Type }
>;
// Row/column share a union member, so their options use that shared type.
type LayoutOptions = Omit<NodeOf<"row" | "column">, "type" | "children">;
export function row(
  children: readonly LayoutChild[],
  options: LayoutOptions = {},
): UiNode {
  return { ...options, type: "row", children };
}
export function column(
  children: readonly LayoutChild[],
  options: LayoutOptions = {},
): UiNode {
  return { ...options, type: "column", children };
}
export function block(
  child: UiNode,
  options: Omit<NodeOf<"block">, "type" | "child"> = {},
): UiNode {
  return { ...options, type: "block", child };
}
export function paragraph(
  lines: readonly TextLine[],
  options: Omit<NodeOf<"paragraph">, "type" | "lines"> = {},
): UiNode {
  return { ...options, type: "paragraph", lines };
}
export function list(
  id: string,
  items: readonly TextLine[],
  options: Omit<NodeOf<"list">, "type" | "id" | "items"> = {},
): UiNode {
  return { ...options, type: "list", id, items };
}
