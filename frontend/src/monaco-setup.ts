import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";
import JsonWorker from "monaco-editor/language/json/json.worker?worker";
import CssWorker from "monaco-editor/language/css/css.worker?worker";
import HtmlWorker from "monaco-editor/language/html/html.worker?worker";
import TsWorker from "monaco-editor/language/typescript/ts.worker?worker";

type MonacoGlobal = {
  MonacoEnvironment?: { getWorker(id: string, label: string): Worker };
};

(self as unknown as MonacoGlobal).MonacoEnvironment = {
  getWorker(_id: string, label: string): Worker {
    switch (label) {
      case "json":
        return new JsonWorker();
      case "css":
      case "scss":
      case "less":
        return new CssWorker();
      case "html":
      case "handlebars":
      case "razor":
        return new HtmlWorker();
      case "typescript":
      case "javascript":
        return new TsWorker();
      default:
        return new EditorWorker();
    }
  },
};

/** App themes layered on Monaco's built-in vs / vs-dark bases. */
export function defineThemes(): void {
  monaco.editor.defineTheme("rn-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#1e1e1e",
      "editorGutter.background": "#1e1e1e",
    },
  });
  monaco.editor.defineTheme("rn-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#ffffff",
      "editorGutter.background": "#ffffff",
    },
  });
}

export { monaco };
