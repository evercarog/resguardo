<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Actividad del cliente: el registro encadenado de todo lo que se hace, con
  // «Verificar la cadena» (en el servidor y, para lo cargado, también aquí) y
  // exportación a CSV.
  import { Download, ShieldCheck, ShieldX, LoaderCircle, Search, Activity, CalendarDays, Clock, UsersRound } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Copiable from "$lib/componentes/Copiable.svelte";
  import { reloj } from "$lib/estado.svelte";
  import { plural, relativo } from "$lib/formato";
  import * as api from "$lib/api";
  import { ApiError } from "$lib/api";
  import { page } from "$app/state";
  import { actual, puede } from "$lib/estado.svelte";
  import { bytes, fechaLarga, numero } from "$lib/formato";
  import { nombreOrden } from "$lib/salud";
  import { primeraRota } from "$lib/auditoria";
  import type { EntradaAuditoria, VerificacionAuditoria } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";

  let entradas = $state<EntradaAuditoria[] | null>(null);
  let error = $state("");
  let buscar = $state("");
  let verificando = $state(false);
  let resultado = $state<{ servidor: VerificacionAuditoria; local: number | null } | null>(null);

  /** La del servidor anterior (importada con el paquete del cliente, §11): se ve aparte. */
  let importada = $state(page.url.searchParams.get("importada") === "1");
  let hayImportada = $state(false);
  $effect(() => {
    // Al entrar directamente, el cliente aún no está cargado: sin id, ni se pregunta.
    if (!actual.id) return;
    void api
      .auditoriaImportada(actual.id, 0, 1)
      .then((x) => (hayImportada = x.length > 0))
      .catch(() => (hayImportada = false));
  });

  let vuelta = 0;
  /** Hay más antiguas (el n más bajo cargado es mayor que 1). */
  const hayMas = $derived(!importada && !!entradas?.length && entradas[entradas.length - 1].n > 1);
  let cargandoMas = $state(false);
  async function cargar(mas = false) {
    if (!actual.id) return;
    const mia = ++vuelta;
    error = "";
    try {
      if (importada) {
        // Entera y de una vez (de la más antigua a la más reciente), y se muestra al revés.
        const todas: EntradaAuditoria[] = [];
        for (let desde = 0; ; ) {
          const p = await api.auditoriaImportada(actual.id, desde, 500);
          todas.push(...p.map((e) => ({ ...e, creado: typeof e.creado === "number" ? new Date((e.creado as number) * 1000).toISOString() : e.creado })));
          if (p.length < 500) break;
          desde = p[p.length - 1].n;
        }
        if (mia === vuelta) entradas = todas.reverse();
        return;
      }
      // De la más reciente hacia atrás, por páginas de 200 (n < antes).
      const antes = mas && entradas?.length ? entradas[entradas.length - 1].n : undefined;
      cargandoMas = mas;
      const p = await api.auditoriaReciente(actual.id, antes, 200);
      if (mia === vuelta) entradas = mas ? [...(entradas ?? []), ...p] : p;
    } catch (e) {
      if (mia !== vuelta) return;
      error = e instanceof ApiError && e.codigo === "prohibido" ? "Tu papel en este cliente no permite ver la actividad." : (e as Error).message;
      if (!mas) entradas = [];
    } finally {
      cargandoMas = false;
    }
  }
  $effect(() => {
    void actual.id;
    void importada;
    entradas = null;
    resultado = null;
    void cargar();
  });

  async function verificar() {
    verificando = true;
    try {
      // La importada ya la comprobó el servidor al aceptarla; aquí se vuelve a comprobar entera.
      const servidor: VerificacionAuditoria = importada ? { ok: true, entradas: entradas?.length ?? 0 } : await api.verificarAuditoria(actual.id);
      resultado = { servidor, local: primeraRota(entradas ?? []) };
    } catch (e) {
      error = (e as Error).message;
    } finally {
      verificando = false;
    }
  }

  const ACCION: Record<string, string> = {
    crear_cliente: "Creó el cliente",
    renombrar_cliente: "Cambió el nombre del cliente",
    invitar: "Invitó a una persona",
    aceptar_invitacion: "Aceptó una invitación",
    poner_rol: "Cambió un papel",
    quitar_miembro: "Quitó a una persona",
    restablecer_totp: "Restableció la verificación en dos pasos de una persona",
    usar_restablecimiento_totp: "Vinculó otro móvil con el código del propietario",
    abrir_emparejamiento: "Abrió un emparejamiento",
    confirmar_emparejamiento: "Confirmó un equipo",
    cancelar_emparejamiento: "Canceló un emparejamiento",
    renombrar_equipo: "Cambió el nombre de un equipo",
    enviar_orden: "Envió una orden",
    orden: "Envió una orden",
    unirse: "Un equipo se unió",
    resultado: "Respuesta de un equipo",
    cancelar_orden: "Canceló una orden",
    importar_cliente: "Importó el historial de otro servidor",
    recibir_cliente: "Recibió el cliente de otro servidor",
    "cliente.crear": "Creó el cliente",
    "miembro.invitar": "Invitó a una persona",
    "miembro.aceptar": "Aceptó una invitación",
    "equipo.emparejar": "Emparejó un equipo",
    "equipo.renombrar": "Cambió el nombre de un equipo",
    "equipo.sin_contacto": "Un equipo dejó de conectarse",
    "equipo.intentos_fallidos": "Intentos fallidos con una clave",
    "orden.rechazada": "Un equipo rechazó una orden",
    "orden.cancelar": "Canceló una orden",
    confirmar_equipo: "Confirmó un equipo",
    ficha_recepcion: "Dio una ficha de este servidor",
    guardar_paquete: "Guardó el historial cifrado",
    equipo_recibido: "Llegó un equipo de otro servidor",
    deja_el_servidor: "Un equipo se fue a otro servidor",
    trasladado: "Un equipo se fue a otro servidor",
    espera_confirmada: "Un equipo confirmó su espera",
    etiqueta_equipo: "Un equipo actualizó su etiqueta (clave de administración nueva)",
    notificaciones_canal: "Cambió un canal de notificaciones",
    notificaciones_preferencias: "Cambió qué avisos recibe una persona",
    entrar: "Entró en la consola",
    entrar_fallido: "Intento de entrada fallido",
    entrar_recuperacion: "Entró con un código de recuperación",
    cambiar_contrasena: "Cambió su contraseña",
    cambiar_autenticador: "Cambió su segundo paso",
    codigos_recuperacion_nuevos: "Generó códigos de recuperación nuevos",
    renombrar_cuenta: "Cambió su nombre",
    inicio: "Primer arranque del servidor",
  };
  /** Nombres de las claves de `datos`, para personas. */
  const CLAVE_DATO: Record<string, string> = {
    seq: "n.º",
    not_before: "se aplica",
    sobre: "sobre",
    espera_min_horas: "espera (h)",
    so: "sistema",
    tipo: "tipo",
    rol: "papel",
    antes: "antes",
    nombre: "nombre",
    usos: "usos",
    dias: "días",
    bytes: "tamaño",
    entradas: "entradas",
    origen: "origen",
    correo: "correo",
  };
  const ROL: Record<string, string> = { propietario: "propietario", administrador: "administrador", tecnico: "técnico", lectura: "solo lectura" };
  const esFecha = (v: unknown) => typeof v === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(v);
  const valor = (k: string, v: unknown): string => {
    if (v == null) return "—";
    if (esFecha(v)) return fechaLarga(v as string);
    if (k === "tipo" && typeof v === "string") return nombreOrden(v);
    if (k === "rol" && typeof v === "string") return ROL[v] ?? v;
    if (k === "bytes" && typeof v === "number") return bytes(v);
    return typeof v === "object" ? JSON.stringify(v) : String(v);
  };
  const accion = (a: string) => ACCION[a] ?? (a.startsWith("orden.") ? `Orden: ${nombreOrden(a.slice(6)).toLowerCase()}` : a.replaceAll("_", " ").replaceAll(".", " · "));
  /** Si el objetivo es un equipo de este cliente, su nombre. */
  const objetivo = (o: string) => actual.equipos.find((e) => e.id === o)?.nombre ?? ROL[o] ?? o;
  const actor = (a: string) => (a.startsWith("cuenta:") ? a.slice(7) : a.startsWith("equipo:") ? `Equipo ${actual.equipos.find((e) => e.id === a.slice(7))?.nombre ?? a.slice(7)}` : a === "servidor" ? "El servidor" : a);
  const datos = (d: string) => {
    try {
      const o = JSON.parse(d);
      if (!o || typeof o !== "object") return String(o ?? "");
      // Primero el tipo de orden; sin los vacíos («se aplica: —»); las huellas largas, cortas (la entera, en el CSV).
      return Object.entries(o)
        .filter(([, v]) => v != null && v !== "")
        .sort(([a], [b]) => Number(b === "tipo") - Number(a === "tipo"))
        .map(([k, v]) => {
          const t = valor(k, v);
          return `${CLAVE_DATO[k] ?? k.replaceAll("_", " ")}: ${/^[0-9a-f]{24,}$/i.test(t) ? `${t.slice(0, 10)}…` : t}`;
        })
        .join(" · ");
    } catch {
      return d;
    }
  };
  // Cifras de lo cargado: hoy, los últimos 7 días, cuántas personas y la última acción.
  const hoyIni = $derived(new Date(new Date(reloj.ahora).setHours(0, 0, 0, 0)).getTime());
  const deHoy = $derived((entradas ?? []).filter((e) => Date.parse(e.creado) >= hoyIni).length);
  const de7 = $derived((entradas ?? []).filter((e) => Date.parse(e.creado) >= reloj.ahora - 7 * 86_400_000).length);
  const personas = $derived(new Set((entradas ?? []).filter((e) => e.actor.startsWith("cuenta:")).map((e) => e.actor)).size);
  const visibles = $derived((entradas ?? []).filter((e) => !buscar.trim() || `${actor(e.actor)} ${accion(e.accion)} ${e.objetivo} ${e.datos}`.toLowerCase().includes(buscar.trim().toLowerCase())));

  function exportar() {
    const esc = (s: string) => `"${String(s).replaceAll('"', '""')}"`;
    const filas = [["n", "fecha", "quién", "acción", "objetivo", "datos", "hash", "hash anterior"].map(esc).join(";")];
    for (const e of [...(entradas ?? [])].reverse()) filas.push([String(e.n), e.creado, e.actor, e.accion, e.objetivo, e.datos, e.hash, e.prev_hash].map(esc).join(";"));
    const url = URL.createObjectURL(new Blob(["﻿" + filas.join("\r\n")], { type: "text/csv;charset=utf-8" }));
    Object.assign(document.createElement("a"), { href: url, download: `actividad-${actual.cliente?.nombre ?? "cliente"}.csv` }).click();
    URL.revokeObjectURL(url);
  }
</script>

<svelte:head><title>Actividad · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina titulo="Actividad" icono={Activity} migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Actividad" }]}>
    {#snippet detalle()}Todo lo que se hace en {actual.cliente?.nombre ?? "el cliente"}, en orden y encadenado: nadie puede borrar ni cambiar una entrada sin que se note. <Ayuda id="auditoria" />{/snippet}
    {#snippet acciones()}
      {#if puede.ordenar(actual.cliente?.rol)}
        <!-- Los técnicos pueden leer la actividad, pero no exportarla. -->
        {#if puede.administrar(actual.cliente?.rol)}<button class="btn" onclick={exportar} disabled={!entradas?.length}><Download size={15} />Exportar CSV</button>{/if}
        <button class="btn btn-primary" onclick={verificar} disabled={verificando || !entradas}>
          {#if verificando}<LoaderCircle size={15} class="spin" />Verificando…{:else}<ShieldCheck size={15} />Verificar la cadena{/if}
        </button>
      {/if}
    {/snippet}
  </CabeceraPagina>

  {#if entradas?.length && !importada}
    <div class="cifras" role="list" aria-label="Cifras de la actividad">
      <Cifra icono={Clock} etiqueta="Hoy" valor={numero(deHoy)} sub={deHoy === 1 ? "acción" : "acciones"} />
      <Cifra icono={CalendarDays} etiqueta="Últimos 7 días" valor={numero(de7)} sub={de7 === 1 ? "acción" : "acciones"} />
      <Cifra icono={UsersRound} etiqueta="Personas" valor={numero(personas)} sub="con alguna acción aquí" />
      <Cifra icono={ShieldCheck} etiqueta="La cadena" valor={resultado ? (resultado.servidor.ok && resultado.local === null ? "Completa" : "Rota") : "Sin verificar"} sub={resultado ? (resultado.servidor.ok && resultado.local === null ? plural(resultado.servidor.ok ? resultado.servidor.entradas : 0, "entrada comprobada", "entradas comprobadas") : "revisa el aviso de arriba") : `última acción ${relativo(entradas[0].creado, reloj.ahora)}`} mal={!!resultado && !(resultado.servidor.ok && resultado.local === null)} />
    </div>
  {/if}

  {#if hayImportada || importada}
    <div class="segmented inline" role="group" aria-label="Qué actividad">
      <button class:on={!importada} aria-pressed={!importada} onclick={() => (importada = false)}>Este servidor</button>
      <button class:on={importada} aria-pressed={importada} onclick={() => (importada = true)}>Servidor anterior</button>
    </div>
    {#if importada}<p class="faint pequeno">La actividad que el cliente trajo de su servidor anterior, tal cual y con su cadena. La de este servidor la enlaza con su última huella (entrada «importar cliente»).</p>{/if}
  {/if}

  {#if resultado}
    {#if resultado.servidor.ok && resultado.local === null}
      <div class="notice notice-success" role="status"><ShieldCheck size={16} /><p>La cadena está completa: {numero(resultado.servidor.ok ? resultado.servidor.entradas : 0)} entradas sin huecos ni cambios. Este navegador ha comprobado también las {numero(entradas?.length ?? 0)} que muestra.</p></div>
    {:else}
      <div class="notice notice-danger" role="alert">
        <ShieldX size={16} />
        <p>
          La cadena está rota en la entrada n.º {!resultado.servidor.ok ? resultado.servidor.rota_en : resultado.local}{resultado.local !== null && resultado.servidor.ok ? " (según este navegador; el servidor dice que está bien)" : ""}. Alguien ha cambiado o borrado entradas: revisa el servidor y avisa a quien lo administre.
        </p>
      </div>
    {/if}
  {/if}

  {#if error}<div class="notice notice-warn"><p>{error}</p></div>{/if}

  {#if entradas === null}
    <Cargando filas={8} />
  {:else if entradas.length}
    <label class="buscar">
      <Search size={15} />
      <input class="input" type="search" placeholder="Buscar por persona, acción o equipo" bind:value={buscar} aria-label="Buscar en la actividad" />
    </label>
    <div class="card p-0 desplazable alto">
      <table class="tabla actividad">
        <caption class="sr-only">Registro de actividad</caption>
        <thead><tr><th scope="col">N.º</th><th scope="col">Cuándo</th><th scope="col">Quién</th><th scope="col">Qué</th><th scope="col">Sobre</th><th scope="col">Huella</th></tr></thead>
        <tbody>
          {#each visibles as e (e.n)}
            <tr>
              <td class="num faint">{e.n}</td>
              <td class="nowrap">{fechaLarga(e.creado)}</td>
              <td>{actor(e.actor)}</td>
              <td>{accion(e.accion)}{#if datos(e.datos)}<br /><span class="faint pequeno">{datos(e.datos)}</span>{/if}</td>
              <td>{objetivo(e.objetivo)}</td>
              <td class="huella"><Copiable texto={e.hash} mostrar="{e.hash.slice(0, 10)}…" que="la huella completa" /></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if hayMas}<button class="btn mas" disabled={cargandoMas} onclick={() => cargar(true)}>{cargandoMas ? "Cargando…" : "Ver más antiguas"}</button>{/if}
  {:else if !error}
    <div class="card"><Vacio icono={Activity} titulo="Sin actividad todavía" texto="Aquí quedará anotado, en orden, todo lo que se haga en el cliente: quién, qué y cuándo." /></div>
  {/if}
</div>

<style>
  .buscar {
    position: relative;
  }
  .buscar :global(svg) {
    position: absolute;
    top: 10px;
    left: 11px;
    color: var(--text-3);
  }
  .buscar .input {
    padding-left: 34px;
  }
  .mas {
    align-self: center;
  }
  .nowrap {
    white-space: nowrap;
  }
  .pequeno {
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  /* Se parte una palabra solo si no cabe (un correo no se corta a media línea);
   * la huella, siempre en una línea. */
  .actividad td {
    overflow-wrap: break-word;
  }
  .actividad td.huella {
    white-space: nowrap;
  }
  /* En el móvil, cada entrada como una tarjeta pequeña. */
  @media (max-width: 640px) {
    .actividad thead {
      display: none;
    }
    .actividad tr {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: 2px 10px;
      padding: 10px 12px;
      border-top: 1px solid var(--border);
    }
    .actividad td {
      padding: 0;
      border: none;
    }
    .actividad td:first-child {
      grid-row: span 5;
    }
    .nowrap {
      white-space: normal;
      color: var(--text-3);
    }
  }
  .huella {
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
