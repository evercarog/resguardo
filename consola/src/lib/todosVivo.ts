// Plan 0.8.0, D4: las secciones de «Todos los clientes» que piden algo aparte del
// panel (avisos, órdenes, restauraciones) lo vuelven a pedir cada 30 s con la
// pestaña visible y al volver a ella. Devuelve cómo parar (para `onMount`).
export function seguirTodos(cargar: () => Promise<void>, ms = 30_000): () => void {
  void cargar();
  const visible = () => typeof document === "undefined" || document.visibilityState === "visible";
  const t = setInterval(() => {
    if (visible()) void cargar();
  }, ms);
  const alVolver = () => {
    if (visible()) void cargar();
  };
  document.addEventListener("visibilitychange", alVolver);
  return () => {
    clearInterval(t);
    document.removeEventListener("visibilitychange", alVolver);
  };
}
