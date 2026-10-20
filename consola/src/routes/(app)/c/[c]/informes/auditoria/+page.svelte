<script lang="ts">
  // 0.8.1 (H2, plan-mejoras 15qq): evidencia para cumplimiento, no un inventario general.
  // (1) El inventario de lo protegido: por equipo y repositorio, responsable, criticidad,
  // si hay datos personales (sí o no), el RTO y el RPO objetivo y el identificador
  // externo de cada equipo; dato común del cliente (`inventario:`), con su CSV.
  // (2) El «Paquete para auditoría» de un periodo, en PDF y CSV, con a qué control
  // responde cada parte (lib/paqueteAuditoria.ts). Es evidencia, no una certificación.
  import { untrack } from "svelte";
  import { FileDown, Pencil, ShieldCheck, Sheet, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import * as api from "$lib/api";
  import { actual, app, cargarCliente, puede, reloj, NOMBRE_ROL } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { cargarFuncionales, funcionalesDe, listaFuncionales } from "$lib/funcionales.svelte";
  import { sincronizar } from "$lib/datosComunes.svelte";
  import { gruposExclusiones } from "$lib/gruposExclusiones";
  import { copiasInforme } from "$lib/informeConfig";
  import { configsDescifradas, bajar } from "$lib/informeConfigCargar";
  import { csvPaquete, documentoPaquete, filasInventario, AVISO_EVIDENCIA, PARTES_PAQUETE, CONTROLES } from "$lib/paqueteAuditoria";
  import { admiteInventario, claveInventario, CRITICIDADES, csvInventario, errorFicha, fichaDe, objetoRepo, textoCriticidad, textoDatosPersonales, textoMin, valorFicha, type FichaInventario, type TipoInventario } from "$lib/inventario";
  import { aMinutos, deMinutos, diaLocal } from "$lib/pruebasFuncionales";
  import { juntar } from "$lib/restauraciones";
  import { pdfDe } from "$lib/pdfTexto";
  import { nombreArchivo } from "$lib/csv";
  import { fechaLarga } from "$lib/formato";
  import type { Configuracion, EntradaHistorial, Equipo, Miembro } from "$lib/tipos";

  const admin = $derived(puede.administrar(actual.cliente?.rol));
  const equiposCliente = $derived(actual.equipos.filter((e) => e.confirmado !== false && e.modo !== "trasladado"));
  $effect(() => {
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    if (!cc) return;
    untrack(() => {
      void cargarInformes(cc, ids);
      void cargarCatalogo(cc);
      void cargarFuncionales(cc);
    });
  });

  // ---------- Inventario ----------
  const inv = $derived(actual.organizacion?.inventario ?? {});
  const filas = $derived(filasInventario(equiposCliente, actual.organizacion));
  const compartido = $derived(admiteInventario(actual.equipos));

  let editando = $state<{ tipo: TipoInventario; objeto: string; titulo: string } | null>(null);
  let ficha = $state<FichaInventario>({});
  let dp = $state<"" | "si" | "no">("");
  let rto = $state({ texto: "", unidad: "h" as "min" | "h" | "dias" });
  let rpo = $state({ texto: "", unidad: "h" as "min" | "h" | "dias" });
  let guardando = $state(false);
  function abrir(tipo: TipoInventario, objeto: string, titulo: string) {
    const f = fichaDe(inv, tipo, objeto);
    ficha = { responsable: f.responsable ?? "", criticidad: f.criticidad ?? null, id_externo: f.id_externo ?? "" };
    dp = f.datos_personales === true ? "si" : f.datos_personales === false ? "no" : "";
    rto = deMinutos(f.rto_min);
    rpo = deMinutos(f.rpo_min);
    editando = { tipo, objeto, titulo };
  }
  const nueva = $derived<FichaInventario>({
    ...ficha,
    datos_personales: dp === "si" ? true : dp === "no" ? false : null,
    rto_min: aMinutos(rto.texto, rto.unidad),
    rpo_min: aMinutos(rpo.texto, rpo.unidad),
  });
  const errorF = $derived(editando ? ([nueva.rto_min, nueva.rpo_min].some((n) => n != null && Number.isNaN(n)) ? "Escribe el RTO y el RPO con números (o déjalos en blanco)." : errorFicha(nueva, editando.tipo)) : null);
  async function guardarFicha() {
    if (!editando || errorF || !actual.id) return;
    guardando = true;
    try {
      const n = { ...nueva, rto_min: nueva.rto_min ? Math.round(nueva.rto_min) : null, rpo_min: nueva.rpo_min ? Math.round(nueva.rpo_min) : null };
      await api.ponerDatosComunes(actual.id, [{ clave: claveInventario(editando.tipo, editando.objeto), valor: valorFicha(n) }]);
      await cargarCliente(actual.id, { silencioso: true });
      void sincronizar().catch(() => {});
      avisar("Inventario guardado.");
      editando = null;
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      guardando = false;
    }
  }
  const linea = (f: FichaInventario) =>
    [f.responsable ? f.responsable : "", f.criticidad ? `Criticidad ${textoCriticidad(f.criticidad).toLowerCase()}` : "", f.datos_personales != null ? `Datos personales: ${textoDatosPersonales(f.datos_personales).toLowerCase()}` : "", f.rto_min ? `RTO ${textoMin(f.rto_min)}` : "", f.rpo_min ? `RPO ${textoMin(f.rpo_min)}` : ""]
      .filter(Boolean)
      .join(" · ") || "Sin anotar";

  async function bajarInventario() {
    if (!actual.cliente) return;
    const hoy = diaLocal(reloj.ahora);
    await bajar(csvInventario(filas), "text/csv;charset=utf-8", nombreArchivo(["Resguardo", actual.cliente.nombre, "inventario de lo protegido", hoy], "csv"));
  }

  // ---------- Paquete ----------
  type Periodo = "30" | "mes" | "90" | "365";
  let periodo = $state<Periodo>("90");
  const rango = $derived.by(() => {
    const hoy = new Date(reloj.ahora);
    const fin = new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() + 1);
    if (periodo === "mes") {
      const desde = new Date(hoy.getFullYear(), hoy.getMonth() - 1, 1);
      return { desde, hasta: new Date(hoy.getFullYear(), hoy.getMonth(), 1) };
    }
    const dias = Number(periodo);
    return { desde: new Date(fin.getFullYear(), fin.getMonth(), fin.getDate() - dias), hasta: fin };
  });
  let clave = $state("");
  let ocupado = $state(false);
  let paso = $state("");
  let faltan = $state<string[]>([]);

  /** El historial del periodo de un equipo (copias, verificaciones y pruebas), página a página. */
  async function historialDe(e: Equipo, desde: Date): Promise<EntradaHistorial[]> {
    const todas: EntradaHistorial[] = [];
    const vistas = new Set<string>();
    let antes: string | undefined;
    for (let i = 0; i < 20; i++) {
      const pag = await api.historialEquipo(actual.id, e.id, { desde: desde.toISOString(), tipo: ["copia", "verificacion", "prueba_restauracion"], limite: 2000, antes });
      const nuevas = pag.filter((x) => !vistas.has(x.id));
      for (const x of nuevas) vistas.add(x.id);
      todas.push(...nuevas);
      if (pag.length < 2000 || !nuevas.length) break;
      antes = pag[pag.length - 1].id;
    }
    return todas;
  }

  async function hacer(formato: "pdf" | "csv") {
    if (!actual.cliente) return;
    ocupado = true;
    faltan = [];
    try {
      let configs: Record<string, Configuracion | null> = {};
      if (clave) {
        const r = await configsDescifradas(actual.cliente, equiposCliente, clave, (t) => (paso = t));
        configs = r.configs;
        faltan = r.faltan;
      }
      const historial: Record<string, EntradaHistorial[]> = {};
      for (const [i, e] of equiposCliente.entries()) {
        paso = `Leyendo el historial de ${e.nombre} (${i + 1} de ${equiposCliente.length})…`;
        try {
          historial[e.id] = await historialDe(e, rango.desde);
        } catch {
          historial[e.id] = [];
          faltan = [...faltan, `${e.nombre}: no se pudo leer su historial.`];
        }
      }
      paso = "Leyendo las restauraciones y las personas…";
      const restauraciones = juntar(await api.restauraciones(actual.id));
      const miembros: Miembro[] = puede.propietario(actual.cliente.rol) ? await api.miembros(actual.id).catch(() => []) : [];
      if (!puede.propietario(actual.cliente.rol)) faltan = [...faltan, "Quién tiene acceso: solo lo ve un propietario del cliente."];
      paso = "Preparando el paquete…";
      const copias = copiasInforme({
        equipos: equiposCliente,
        todos: actual.equipos,
        informes: ultimos.porEquipo,
        catalogo: catalogoDe(actual.id),
        organizacion: actual.organizacion,
        funcionales: listaFuncionales(actual.id),
        infra: actual.infra,
        configs,
        grupos: gruposExclusiones(actual),
        ahora: reloj.ahora,
      });
      const generado = `Generado el ${fechaLarga(new Date(reloj.ahora).toISOString())}${app.cuenta?.nombre ? ` por ${app.cuenta.nombre}` : ""}`;
      const d = {
        cliente: actual.cliente.nombre,
        desde: rango.desde,
        hasta: rango.hasta,
        generado,
        equipos: equiposCliente,
        organizacion: actual.organizacion,
        copias,
        conClave: !!clave,
        historial,
        restauraciones,
        funcionales: listaFuncionales(actual.id),
        politica: funcionalesDe(actual.id)?.ajustes ?? null,
        miembros,
        nombreRol: (r: string) => NOMBRE_ROL[r as keyof typeof NOMBRE_ROL] ?? r,
        ahora: reloj.ahora,
      };
      const nombre = nombreArchivo(["Resguardo", actual.cliente.nombre, "paquete para auditoría", diaLocal(rango.desde), diaLocal(rango.hasta.getTime() - 1)], formato);
      if (formato === "csv") await bajar(csvPaquete(d), "text/csv;charset=utf-8", nombre);
      else await bajar(pdfDe(documentoPaquete(d)) as BlobPart, "application/pdf", nombre);
      avisar(formato === "pdf" ? "Paquete en PDF listo." : "Paquete en CSV listo.");
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      clave = "";
      paso = "";
      ocupado = false;
    }
  }
</script>

<svelte:head><title>Auditoría · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Auditoría"
    icono={ShieldCheck}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Informes", href: `/c/${actual.id}/informes` }, { texto: "Auditoría" }]}
    resumen="Evidencia de lo que se protege y de cómo se ha protegido, para quien audita la seguridad o la protección de datos del cliente. No es una certificación."
  />

  {#if !admin}
    <div class="notice notice-info"><p>Esta página es para administradores del cliente.</p></div>
  {:else}
    <section class="card p" aria-labelledby="t-inv">
      <div class="cab">
        <h2 class="section-title" id="t-inv">Inventario de lo protegido</h2>
        <button class="btn btn-sm" onclick={bajarInventario}><Sheet size={14} />Inventario en CSV</button>
      </div>
      <p class="faint">Opcional. Quién responde de cada equipo y repositorio, cuánto importa, si guarda datos personales (solo sí o no, nunca cuáles) y en cuánto tiempo hay que recuperarlo. El identificador externo es la etiqueta del inventario que ya use el cliente, para enlazarlo. Nada de números de serie, usuarios ni programas.</p>
      {#if !compartido}
        <div class="notice notice-info"><p>Se guarda en esta consola. Para verlo igual en todas las consolas del cliente, actualiza sus agentes.</p></div>
      {/if}
      <ul class="inv">
        {#each equiposCliente as e (e.id)}
          {@const fe = fichaDe(inv, "equipo", e.id)}
          <li class="equipo">
            <div class="fila-inv">
              <span class="nombre"><strong>{e.nombre}</strong>{#if fe.id_externo}<span class="faint"> · {fe.id_externo}</span>{/if}</span>
              <span class="faint resumen">{linea(fe)}</span>
              <button class="btn btn-ghost btn-sm" aria-label="Anotar {e.nombre}" onclick={() => abrir("equipo", e.id, e.nombre)}><Pencil size={14} />Anotar</button>
            </div>
            {#if e.resumen?.repositorios?.length}
              <ul>
                {#each e.resumen.repositorios as r (r.id)}
                  {@const fr = fichaDe(inv, "repositorio", objetoRepo(e.id, r.id))}
                  <li class="fila-inv">
                    <span class="nombre">{r.nombre}</span>
                    <span class="faint resumen">{linea(fr)}</span>
                    <button class="btn btn-ghost btn-sm" aria-label="Anotar {r.nombre} de {e.nombre}" onclick={() => abrir("repositorio", objetoRepo(e.id, r.id), `${r.nombre} (${e.nombre})`)}><Pencil size={14} />Anotar</button>
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <section class="card p" aria-labelledby="t-paq">
      <h2 class="section-title" id="t-paq">Paquete para auditoría</h2>
      <div class="notice notice-info"><p>{AVISO_EVIDENCIA}</p></div>
      <form
        class="form"
        onsubmit={(e) => {
          e.preventDefault();
          void hacer("pdf");
        }}
      >
        <div class="segmented inline" role="group" aria-label="Periodo">
          <button type="button" class:on={periodo === "30"} aria-pressed={periodo === "30"} onclick={() => (periodo = "30")}>Últimos 30 días</button>
          <button type="button" class:on={periodo === "mes"} aria-pressed={periodo === "mes"} onclick={() => (periodo = "mes")}>Mes pasado</button>
          <button type="button" class:on={periodo === "90"} aria-pressed={periodo === "90"} onclick={() => (periodo = "90")}>Últimos 3 meses</button>
          <button type="button" class:on={periodo === "365"} aria-pressed={periodo === "365"} onclick={() => (periodo = "365")}>Último año</button>
        </div>
        <p class="hint">Del {fechaLarga(rango.desde.toISOString())} al {fechaLarga(new Date(rango.hasta.getTime() - 1).toISOString())}.</p>
        <details class="partes">
          <summary>Qué lleva y a qué control responde cada parte</summary>
          <ul>
            {#each PARTES_PAQUETE as p (p.id)}<li><strong>{p.titulo}</strong>: <span class="faint">{p.controles.map((c) => CONTROLES[c]).join(" · ")}</span></li>{/each}
          </ul>
        </details>
        <CampoClave
          id="paq-clave"
          etiqueta="Clave de administración (opcional)"
          bind:value={clave}
          ayuda="Con la clave, el paquete lleva también las carpetas, lo que no se copia y lo de «antes de copiar» de cada copia (se abren aquí, en tu navegador)."
        />
        {#if faltan.length}
          <div class="notice notice-warn" role="status"><TriangleAlert size={16} /><p>{faltan.join(" ")}</p></div>
        {/if}
        <div class="acciones">
          <button type="submit" class="btn btn-primary" disabled={ocupado}><FileDown size={16} />Descargar en PDF</button>
          <button type="button" class="btn" disabled={ocupado} onclick={() => hacer("csv")}><Sheet size={16} />Descargar en CSV</button>
          {#if ocupado}<span class="faint" role="status">{paso}</span>{/if}
        </div>
      </form>
    </section>
  {/if}
</div>

{#if editando}
  <Modal labelledby="t-ficha-inv" onclose={() => (editando = null)} dismissible={false} ocupado={guardando}>
    <div class="dlg-title">
      <span class="ticon"><ShieldCheck size={18} /></span>
      <div>
        <h2 id="t-ficha-inv">Inventario de {editando.titulo}</h2>
        <p>Todo opcional. Se ve igual en todas las consolas del cliente.</p>
      </div>
    </div>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        void guardarFicha();
      }}
    >
      <div class="field">
        <label class="field-label" for="inv-resp">Responsable</label>
        <input id="inv-resp" class="input" maxlength="80" bind:value={ficha.responsable} placeholder="Ana Pérez (Administración)" autocomplete="off" />
      </div>
      <div class="dos">
        <div class="field">
          <label class="field-label" for="inv-crit">Criticidad</label>
          <select id="inv-crit" class="input" bind:value={ficha.criticidad}>
            <option value={null}>Sin anotar</option>
            {#each CRITICIDADES as c (c.id)}<option value={c.id}>{c.texto}</option>{/each}
          </select>
        </div>
        <div class="field">
          <label class="field-label" for="inv-dp">¿Hay datos personales?</label>
          <select id="inv-dp" class="input" bind:value={dp}>
            <option value="">Sin anotar</option>
            <option value="si">Sí</option>
            <option value="no">No</option>
          </select>
        </div>
      </div>
      <div class="dos">
        <div class="field">
          <label class="field-label" for="inv-rto">RTO objetivo <span class="faint">(de vuelta en…)</span></label>
          <div class="fila-campo">
            <input id="inv-rto" class="input num mini" inputmode="decimal" bind:value={rto.texto} placeholder="—" />
            <select class="input" bind:value={rto.unidad} aria-label="Unidad del RTO"><option value="min">minutos</option><option value="h">horas</option><option value="dias">días</option></select>
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="inv-rpo">RPO objetivo <span class="faint">(perder como mucho…)</span></label>
          <div class="fila-campo">
            <input id="inv-rpo" class="input num mini" inputmode="decimal" bind:value={rpo.texto} placeholder="—" />
            <select class="input" bind:value={rpo.unidad} aria-label="Unidad del RPO"><option value="min">minutos</option><option value="h">horas</option><option value="dias">días</option></select>
          </div>
        </div>
      </div>
      {#if editando.tipo === "equipo"}
        <div class="field">
          <label class="field-label" for="inv-id">Identificador externo <span class="faint">(la etiqueta de su inventario)</span></label>
          <input id="inv-id" class="input" maxlength="64" bind:value={ficha.id_externo} placeholder="INV-0042" autocomplete="off" />
        </div>
      {/if}
      <p class="hint">Solo si hay datos personales, nunca cuáles. No pongas contraseñas aquí.</p>
      {#if errorF}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{errorF}</p></div>{/if}
      <footer class="pie">
        <span class="hueco"></span>
        <button type="button" class="btn btn-ghost" disabled={guardando} onclick={() => (editando = null)}>Cancelar</button>
        <button type="submit" class="btn btn-primary" disabled={guardando || !!errorF}>Guardar</button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .cab {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }
  .inv,
  .inv ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .inv > li + li {
    border-top: 1px solid var(--border);
  }
  .inv ul {
    padding-left: var(--sp-5);
  }
  .fila-inv {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 6px 0;
    flex-wrap: wrap;
    min-width: 0;
  }
  .fila-inv .nombre {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .fila-inv .resumen {
    flex: 1;
    min-width: 12rem;
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .partes ul {
    margin: var(--sp-2) 0 0;
    padding-left: var(--sp-5);
    font-size: var(--fs-sm);
  }
  .dos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 200px), 1fr));
    gap: var(--sp-4);
  }
  .fila-campo {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .fila-campo .input:not(.mini) {
    flex: 1;
    min-width: 0;
  }
  .mini {
    width: 5.5rem;
  }
  .pie {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .hueco {
    flex: 1;
  }
</style>
