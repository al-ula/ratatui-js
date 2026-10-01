import type {
  ChartAxis,
  ChartDataset,
  Constraint,
  LayoutChild,
  TextLine,
  UiNode,
} from "@ratatui-js/protocol";
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

export function table(
  id: string,
  rows: readonly (readonly TextLine[])[],
  widths: readonly Constraint[],
  options: Omit<NodeOf<"table">, "type" | "id" | "rows" | "widths"> = {},
): UiNode {
  return { ...options, type: "table", id, rows, widths };
}
export function tabs(
  id: string,
  titles: readonly TextLine[],
  options: Omit<NodeOf<"tabs">, "type" | "id" | "titles"> = {},
): UiNode {
  return { ...options, type: "tabs", id, titles };
}
export function gauge(
  ratio: number,
  options: Omit<NodeOf<"gauge">, "type" | "ratio"> = {},
): UiNode {
  return { ...options, type: "gauge", ratio };
}
export function chart(
  datasets: readonly ChartDataset[],
  xAxis: ChartAxis,
  yAxis: ChartAxis,
  options: Omit<NodeOf<"chart">, "type" | "datasets" | "xAxis" | "yAxis"> = {},
): UiNode {
  return { ...options, type: "chart", datasets, xAxis, yAxis };
}
export function scrollbar(
  id: string,
  contentLength: number,
  options: Omit<NodeOf<"scrollbar">, "type" | "id" | "contentLength"> = {},
): UiNode {
  return { ...options, type: "scrollbar", id, contentLength };
}
