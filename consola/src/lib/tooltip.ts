// Tooltips de verdad (docs/diseno.md §4), en vez del `title` nativo, que no
// sale con el teclado ni en el móvil y tarda lo que quiere cada navegador.
//
//   <button use:tip={"Copiar la línea"} aria-label="Copiar la línea">…</button>
//   <time use:tip={fechaLarga(iso)}>hace 3 h</time>
//
// - Ratón: aparece a los 400 ms de quedarse encima y se va al salir.
// - Teclado: aparece al enfocar con el teclado (`:focus-visible`) y se va al
//   salir o con Esc.
// - Táctil: en lo que no es un botón ni un enlace, un toque lo enseña unos
//   segundos; en botones y enlaces, una pulsación larga (el toque normal hace
//   su acción).
// - Lectores de pantalla: el texto va siempre en un elemento oculto enlazado
//   con `aria-describedby` (salvo que diga lo mismo que el `aria-label`).
//
// Hay un solo globo para toda la página, al final de <body>: así ningún
// contenedor con recorte o transformación lo corta ni lo descoloca.

type Opciones = string | null | undefined | { texto: string | null | undefined; lado?: "arriba" | "abajo" };

const RETARDO = 400;
const TOQUE_LARGO = 450;
const DURA_TOQUE = 2500;
let globo: HTMLDivElement | null = null;
let ocultas: HTMLDivElement | null = null;
let duenoActual: HTMLElement | null = null;
let n = 0;

function asegurar() {
  if (globo || typeof document === "undefined") return;
  globo = document.createElement("div");
  globo.className = "rg-tooltip";
  globo.setAttribute("role", "tooltip");
  globo.setAttribute("aria-hidden", "true");
  document.body.appendChild(globo);
  ocultas = document.createElement("div");
  ocultas.hidden = true;
  ocultas.className = "rg-tooltip-textos";
  document.body.appendChild(ocultas);
  // Al desplazar o cambiar el tamaño, el globo se quita (si no, se quedaría flotando fuera de sitio).
  const quitar = () => ocultar();
  window.addEventListener("scroll", quitar, { capture: true, passive: true });
  window.addEventListener("resize", quitar, { passive: true });
}

function mostrar(dueno: HTMLElement, texto: string, lado: "arriba" | "abajo") {
  asegurar();
  if (!globo || !texto) return;
  duenoActual = dueno;
  globo.textContent = texto;
  globo.dataset.visible = "1";
  // Medir con el texto puesto y colocarlo: arriba (o abajo si no cabe), centrado y dentro de la pantalla.
  const r = dueno.getBoundingClientRect();
  const g = globo.getBoundingClientRect();
  const margen = 8;
  let arriba = lado === "arriba";
  if (arriba && r.top - g.height - margen < 4) arriba = false;
  else if (!arriba && r.bottom + g.height + margen > window.innerHeight - 4) arriba = true;
  const top = arriba ? r.top - g.height - margen : r.bottom + margen;
  const left = Math.max(8, Math.min(window.innerWidth - g.width - 8, r.left + r.width / 2 - g.width / 2));
  globo.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
  globo.dataset.lado = arriba ? "arriba" : "abajo";
}

function ocultar(dueno?: HTMLElement) {
  if (!globo || (dueno && duenoActual !== dueno)) return;
  delete globo.dataset.visible;
  duenoActual = null;
}

const esInteractivo = (el: HTMLElement) => el.matches("a[href], button, input, select, textarea, summary, [role='button'], [tabindex]:not([tabindex='-1'])");

export function tip(node: HTMLElement, opciones: Opciones) {
  let texto = "";
  let lado: "arriba" | "abajo" = "arriba";
  let espera: ReturnType<typeof setTimeout> | undefined;
  let fin: ReturnType<typeof setTimeout> | undefined;
  let desc: HTMLSpanElement | null = null;
  const id = `rg-tip-${++n}`;

  function poner(o: Opciones) {
    const t = typeof o === "string" || o == null ? o : o.texto;
    texto = (t ?? "").trim();
    lado = typeof o === "object" && o && o.lado ? o.lado : "arriba";
    asegurar();
    // La descripción para lectores de pantalla (oculta), si añade algo al nombre.
    const repite = !texto || texto === node.getAttribute("aria-label")?.trim();
    if (repite) {
      desc?.remove();
      desc = null;
      quitarDescribedby();
    } else if (ocultas) {
      desc ??= Object.assign(document.createElement("span"), { id });
      desc.textContent = texto;
      if (!desc.isConnected) ocultas.appendChild(desc);
      const ids = (node.getAttribute("aria-describedby") ?? "").split(/\s+/).filter(Boolean);
      if (!ids.includes(id)) node.setAttribute("aria-describedby", [...ids, id].join(" "));
    }
    if (duenoActual === node) texto ? mostrar(node, texto, lado) : ocultar(node);
  }
  function quitarDescribedby() {
    const ids = (node.getAttribute("aria-describedby") ?? "").split(/\s+/).filter((x) => x && x !== id);
    if (ids.length) node.setAttribute("aria-describedby", ids.join(" "));
    else node.removeAttribute("aria-describedby");
  }

  const cancelar = () => {
    clearTimeout(espera);
    clearTimeout(fin);
  };
  const entrar = (e: PointerEvent) => {
    if (e.pointerType === "touch" || !texto) return;
    cancelar();
    espera = setTimeout(() => mostrar(node, texto, lado), RETARDO);
  };
  const salir = () => {
    cancelar();
    ocultar(node);
  };
  const enfocar = () => {
    // Solo con el teclado: un clic con el ratón no debe dejar el globo puesto.
    if (!texto || !node.matches(":focus-visible")) return;
    cancelar();
    espera = setTimeout(() => mostrar(node, texto, lado), 150);
  };
  const tecla = (e: KeyboardEvent) => {
    if (e.key === "Escape" && duenoActual === node) {
      ocultar(node);
      e.stopPropagation();
    }
  };
  // Táctil.
  const tocar = (e: PointerEvent) => {
    if (e.pointerType !== "touch" || !texto) return;
    cancelar();
    if (esInteractivo(node)) {
      espera = setTimeout(() => {
        mostrar(node, texto, lado);
        fin = setTimeout(() => ocultar(node), DURA_TOQUE);
      }, TOQUE_LARGO);
    } else {
      if (duenoActual === node) return ocultar(node);
      mostrar(node, texto, lado);
      fin = setTimeout(() => ocultar(node), DURA_TOQUE);
    }
  };
  const soltar = (e: PointerEvent) => {
    if (e.pointerType === "touch") clearTimeout(espera);
  };

  // Sin `title` nativo: saldrían dos globos. `data-tip`: los botones de icono ya no lo piden por su cuenta.
  if (node.hasAttribute("title")) node.removeAttribute("title");
  node.dataset.tip = "";
  poner(opciones);
  node.addEventListener("pointerenter", entrar);
  node.addEventListener("pointerleave", salir);
  node.addEventListener("pointerdown", tocar);
  node.addEventListener("pointerup", soltar);
  node.addEventListener("pointercancel", soltar);
  node.addEventListener("focus", enfocar);
  node.addEventListener("blur", salir);
  node.addEventListener("keydown", tecla);
  // Con el ratón, al pulsar se quita (la acción ya habla por sí sola).
  const pulsar = () => {
    if (duenoActual === node && !node.matches(":focus-visible")) salir();
  };
  node.addEventListener("click", pulsar);

  return {
    update: poner,
    destroy() {
      cancelar();
      ocultar(node);
      desc?.remove();
      quitarDescribedby();
      node.removeEventListener("pointerenter", entrar);
      node.removeEventListener("pointerleave", salir);
      node.removeEventListener("pointerdown", tocar);
      node.removeEventListener("pointerup", soltar);
      node.removeEventListener("pointercancel", soltar);
      node.removeEventListener("focus", enfocar);
      node.removeEventListener("blur", salir);
      node.removeEventListener("keydown", tecla);
      node.removeEventListener("click", pulsar);
    },
  };
}

/**
 * Los botones de solo icono (`.icon-btn` con `aria-label`) enseñan su nombre
 * en un tooltip sin tener que ponérselo uno a uno: se escucha en el documento
 * (ratón a los 400 ms, teclado al enfocar). Su nombre ya lo leen los lectores
 * de pantalla, así que no hace falta `aria-describedby`.
 */
export function tooltipsDeIconos(): () => void {
  if (typeof document === "undefined") return () => {};
  const SEL = ".icon-btn[aria-label]:not([data-tip])";
  let espera: ReturnType<typeof setTimeout> | undefined;
  const sobre = (e: PointerEvent) => {
    if (e.pointerType === "touch") return;
    const b = (e.target as Element | null)?.closest?.(SEL) as HTMLElement | null;
    if (!b || b === duenoActual) return;
    clearTimeout(espera);
    espera = setTimeout(() => b.matches(":hover") && mostrar(b, b.getAttribute("aria-label") ?? "", "arriba"), RETARDO);
  };
  const fuera = (e: PointerEvent) => {
    const b = (e.target as Element | null)?.closest?.(SEL) as HTMLElement | null;
    if (!b || b.contains(e.relatedTarget as Node | null)) return;
    clearTimeout(espera);
    ocultar(b);
  };
  const foco = (e: FocusEvent) => {
    const b = (e.target as Element | null)?.closest?.(SEL) as HTMLElement | null;
    if (b && b.matches(":focus-visible")) mostrar(b, b.getAttribute("aria-label") ?? "", "arriba");
  };
  const sinFoco = (e: FocusEvent) => {
    const b = (e.target as Element | null)?.closest?.(SEL) as HTMLElement | null;
    if (b) ocultar(b);
  };
  const pulsar = (e: Event) => {
    clearTimeout(espera);
    const b = (e.target as Element | null)?.closest?.(SEL) as HTMLElement | null;
    if (b && !b.matches(":focus-visible")) ocultar(b);
  };
  const esc = (e: KeyboardEvent) => {
    if (e.key === "Escape" && duenoActual && !duenoActual.hasAttribute("data-tip")) ocultar();
  };
  document.addEventListener("pointerover", sobre);
  document.addEventListener("pointerout", fuera);
  document.addEventListener("focusin", foco);
  document.addEventListener("focusout", sinFoco);
  document.addEventListener("pointerdown", pulsar, true);
  document.addEventListener("keydown", esc);
  return () => {
    clearTimeout(espera);
    document.removeEventListener("pointerover", sobre);
    document.removeEventListener("pointerout", fuera);
    document.removeEventListener("focusin", foco);
    document.removeEventListener("focusout", sinFoco);
    document.removeEventListener("pointerdown", pulsar, true);
    document.removeEventListener("keydown", esc);
  };
}
