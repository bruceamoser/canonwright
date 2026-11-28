import { useEffect, useRef, useState } from "react";

import { GlobalWorkerOptions, getDocument } from "pdfjs-dist/legacy/build/pdf.mjs";
import type { PDFDocumentProxy } from "pdfjs-dist";
import workerSource from "pdfjs-dist/legacy/build/pdf.worker.min.mjs?url";

GlobalWorkerOptions.workerSrc = workerSource;

type Zoom = "width" | "window" | number;
const cssPixelsPerPoint = 96 / 72;

function decodeDataUrl(dataUrl: string): Uint8Array {
  const separator = dataUrl.indexOf(",");
  const base64 = separator >= 0 ? dataUrl.slice(separator + 1) : dataUrl;
  const binary = window.atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

export default function PdfViewer({
  dataUrl,
  title,
}: {
  dataUrl: string;
  title: string;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [document, setDocument] = useState<PDFDocumentProxy | null>(null);
  const [pageNumber, setPageNumber] = useState(1);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [zoom, setZoom] = useState<Zoom>("width");
  const [shownScale, setShownScale] = useState(0);
  const [error, setError] = useState("");
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let active = true;
    setDocument(null);
    setPageNumber(1);
    setZoom("width");
    setShownScale(0);
    setError("");
    const task = getDocument({ data: decodeDataUrl(dataUrl) });
    task.promise.then(pdf => { if (active) setDocument(pdf); })
      .catch(cause => { if (active) setError(String(cause)); });
    return () => { active = false; void task.destroy(); };
  }, [dataUrl]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(([entry]) => setSize({ width: Math.floor(entry.contentRect.width), height: Math.floor(entry.contentRect.height) }));
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!document || !size.width || !size.height) return;
    let active = true;
    let renderTask: ReturnType<Awaited<ReturnType<PDFDocumentProxy["getPage"]>>["render"]> | undefined;
    setError("");
    void document.getPage(pageNumber).then(async page => {
      const canvas = canvasRef.current;
      if (!active || !canvas) return;
      const base = page.getViewport({ scale: cssPixelsPerPoint });
      const widthScale = size.width / base.width;
      const scale = typeof zoom === "number" ? zoom : zoom === "width" ? widthScale
        : Math.min(widthScale, size.height / base.height);
      setShownScale(scale);
      const viewport = page.getViewport({ scale: scale * cssPixelsPerPoint });
      const ratio = Math.min(2, window.devicePixelRatio || 1, 8192 / Math.max(viewport.width, viewport.height),
        Math.sqrt(16_000_000 / (viewport.width * viewport.height)));
      canvas.style.width = Math.floor(viewport.width) + "px";
      canvas.style.height = Math.floor(viewport.height) + "px";
      canvas.width = Math.ceil(viewport.width * ratio);
      canvas.height = Math.ceil(viewport.height * ratio);
      renderTask = page.render({ canvas, viewport, transform: [ratio, 0, 0, ratio, 0, 0] });
      await renderTask.promise;
    }).catch(cause => { if (active) setError(String(cause)); });
    return () => { active = false; renderTask?.cancel(); };
  }, [document, pageNumber, size, zoom]);

  return <div className="flex h-full min-h-0 flex-col gap-2">
    {document && <nav aria-label="PDF pages" className="flex shrink-0 flex-wrap items-center gap-1">
      <button disabled={pageNumber === 1} onClick={() => setPageNumber(n => n - 1)}>Previous</button>
      <span>Page {pageNumber} of {document.numPages}</span>
      <button disabled={pageNumber === document.numPages} onClick={() => setPageNumber(n => n + 1)}>Next</button>
      <button aria-label="Zoom out" title="Zoom out" disabled={!shownScale || shownScale <= 0.1}
        onClick={() => setZoom(Math.max(0.1, shownScale / 1.25))}>−</button>
      <span className="tabular-nums">{Math.round(shownScale * 100)}%</span>
      <button aria-label="Zoom in" title="Zoom in" disabled={!shownScale || shownScale >= 8}
        onClick={() => setZoom(Math.min(8, shownScale * 1.25))}>+</button>
      <button aria-pressed={zoom === "width"} onClick={() => setZoom("width")}>Fit width</button>
      <button aria-pressed={zoom === "window"} onClick={() => setZoom("window")}>Fit window</button>
    </nav>}
    {error ? <p role="alert">{error}</p> : !document && <p role="status">Loading PDF…</p>}
    <div className="min-h-0 flex-1 overflow-auto rounded bg-obd-inset p-2" ref={containerRef} data-testid="pdf-pages">
      {document && <canvas ref={canvasRef} key={pageNumber} aria-label={`${title}, page ${pageNumber}`} className="mx-auto block max-w-none bg-white" />}
    </div>
  </div>;
}
