// Plan 0.8.1, B1 (15bb): el menú de la consola en un solo sitio. Unas ocho
// secciones (y «Ajustes» abajo, aparte), algunas con pestañas; cada pestaña
// con su dirección. Lo usan la barra lateral, las pestañas de cada sección, la
// paleta (Ctrl K) y los atajos. Cada papel ve solo lo suyo: una pestaña que
// no puede usar no sale, y una sección sin pestañas tampoco.
import {
  Activity,
  ArchiveRestore,
  BellRing,
  Building2,
  ClipboardCheck,
  ClipboardList,
  Database,
  FileBarChart,
  FileSearch,
  FolderSync,
  History,
  LayoutDashboard,
  Monitor,
  PackageCheck,
  ArrowRightLeft,
  ScrollText,
  SlidersHorizontal,
  Users,
} from "@lucide/svelte";
import type { Rol } from "./tipos";

type Icono = typeof LayoutDashboard;

export interface PestanaDef {
  /** Dirección dentro del cliente (`/recuperar/buscar`). */
  ruta: string;
  texto: string;
  icono: Icono;
  /** Solo esta dirección (no lo de debajo). */
  exacto?: boolean;
  /** Quién la ve (sin esto, todos). */
  puede?: (rol: Rol | undefined) => boolean;
  /** Palabras de más para el buscador (sin tildes, en minúsculas). */
  claves?: string;
}

export interface SeccionDef {
  id: "estado" | "equipos" | "copias" | "destinos" | "recuperar" | "avisos" | "actividad" | "informes" | "ajustes";
  texto: string;
  icono: Icono;
  /** Lo que la marca como activa (`""` solo para Estado, que es exacta). */
  prefijo: string;
  pestanas: PestanaDef[];
  /** Abajo y aparte (Ajustes). */
  abajo?: boolean;
}

const ordenar = (r: Rol | undefined) => r === "propietario" || r === "administrador" || r === "tecnico";
const administrar = (r: Rol | undefined) => r === "propietario" || r === "administrador";
const propietario = (r: Rol | undefined) => r === "propietario";

/** Las secciones del cliente, en orden (las pestañas, en el suyo: la primera es la de entrada). */
export const SECCIONES: SeccionDef[] = [
  { id: "estado", texto: "Estado", icono: LayoutDashboard, prefijo: "", pestanas: [{ ruta: "", texto: "Estado", icono: LayoutDashboard, exacto: true, claves: "inicio resumen panel mapa" }] },
  { id: "equipos", texto: "Equipos", icono: Monitor, prefijo: "/equipos", pestanas: [{ ruta: "/equipos", texto: "Equipos", icono: Monitor, claves: "ordenadores pc servidores" }] },
  { id: "copias", texto: "Copias", icono: FolderSync, prefijo: "/copias", pestanas: [{ ruta: "/copias", texto: "Copias", icono: FolderSync, claves: "copias de seguridad horarios" }] },
  {
    id: "destinos",
    texto: "Destinos",
    icono: Database,
    prefijo: "/destinos",
    pestanas: [
      { ruta: "/destinos", texto: "Destinos", icono: Database, claves: "repositorios y destinos almacen nube discos" },
      // Plan 0.8.1, C4 (15d): dónde están de verdad los datos (servidores, discos, hipervisores).
      { ruta: "/destinos/infraestructura", texto: "Infraestructura", icono: ServerCog, claves: "infraestructura servidores fisicos discos raid zfs hipervisor" },
    ],
  },
  {
    id: "recuperar",
    texto: "Recuperar",
    icono: History,
    prefijo: "/recuperar",
    pestanas: [
      { ruta: "/recuperar", texto: "Restaurar", icono: History, exacto: true, claves: "restaurar archivos recuperar" },
      { ruta: "/recuperar/buscar", texto: "Buscar archivos", icono: FileSearch, claves: "buscar archivos en las copias" },
      { ruta: "/recuperar/restauraciones", texto: "Restauraciones", icono: ArchiveRestore, claves: "historial de restauraciones registro rto" },
      { ruta: "/recuperar/pruebas", texto: "Pruebas funcionales", icono: ClipboardCheck, claves: "simulacros pruebas funcionales recuperacion rto rpo" },
    ],
  },
  { id: "avisos", texto: "Avisos", icono: BellRing, prefijo: "/avisos", pestanas: [{ ruta: "/avisos", texto: "Avisos", icono: BellRing, claves: "alertas notificaciones" }] },
  {
    id: "actividad",
    texto: "Actividad",
    icono: Activity,
    prefijo: "/actividad",
    pestanas: [
      { ruta: "/actividad", texto: "Órdenes", icono: ClipboardList, exacto: true, claves: "ordenes en espera cancelar" },
      { ruta: "/actividad/registro", texto: "Registro", icono: ScrollText, puede: ordenar, claves: "actividad auditoria registro quien hizo que" },
    ],
  },
  { id: "informes", texto: "Informes", icono: FileBarChart, prefijo: "/informes", pestanas: [{ ruta: "/informes", texto: "Informes", icono: FileBarChart, claves: "informe imprimir pdf ejecutivo" }] },
  {
    id: "ajustes",
    texto: "Ajustes",
    icono: SlidersHorizontal,
    prefijo: "/ajustes",
    abajo: true,
    pestanas: [
      { ruta: "/ajustes/personas", texto: "Personas y ajustes", icono: Users, puede: propietario, claves: "personas cuentas invitar ajustes del cliente notificaciones marca" },
      { ruta: "/ajustes/organizacion", texto: "Organización", icono: Building2, puede: ordenar, claves: "organizacion sedes categorias etiquetas grupos de exclusiones no copiar" },
      { ruta: "/ajustes/versiones", texto: "Versiones del agente", icono: PackageCheck, claves: "versiones del agente actualizaciones anillos" },
      { ruta: "/ajustes/servidor", texto: "Servidor", icono: ArrowRightLeft, puede: administrar, claves: "servidor consolas conectar codigo de conexion mover copia de la consola" },
    ],
  },
];

/** Las pestañas que ve este papel. */
export const pestanasDe = (s: SeccionDef, rol: Rol | undefined) => s.pestanas.filter((p) => !p.puede || p.puede(rol));

/** Las secciones que ve este papel (sin las que se quedan sin pestañas). */
export const seccionesPara = (rol: Rol | undefined) => SECCIONES.filter((s) => pestanasDe(s, rol).length);

/** ¿Está `ruta` (dentro del cliente: `/recuperar/buscar`) en esta pestaña? */
export const enPestana = (p: Pick<PestanaDef, "ruta" | "exacto">, ruta: string) => (p.exacto || !p.ruta ? ruta === p.ruta : ruta === p.ruta || ruta.startsWith(p.ruta + "/"));

/** ¿Está `ruta` en esta sección? */
export const enSeccion = (s: SeccionDef, ruta: string) => (s.prefijo ? ruta === s.prefijo || ruta.startsWith(s.prefijo + "/") : ruta === "");

/** La ruta dentro del cliente de una dirección (`/c/x/recuperar` → `/recuperar`), o `null` si no es de ese cliente. */
export function rutaEnCliente(pathname: string, cliente: string): string | null {
  const base = `/c/${cliente}`;
  if (pathname === base || pathname === base + "/") return "";
  return pathname.startsWith(base + "/") ? pathname.slice(base.length).replace(/\/$/, "") : null;
}

/** La pestaña activa de una sección (la más concreta que encaja), o la primera. */
export function pestanaActiva(ps: PestanaDef[], ruta: string): number {
  let mejor = -1;
  ps.forEach((p, i) => {
    if (enPestana(p, ruta) && (mejor < 0 || p.ruta.length > ps[mejor].ruta.length)) mejor = i;
  });
  return mejor;
}

/** «Todos los clientes» con el mismo criterio: las secciones y sus pestañas. */
export interface SeccionTodos {
  id: string;
  texto: string;
  icono: Icono;
  pestanas: { href: string; texto: string; icono: Icono; exacto?: boolean }[];
}

export const SECCIONES_TODOS: SeccionTodos[] = [
  { id: "estado", texto: "Estado", icono: LayoutDashboard, pestanas: [{ href: "/todos", texto: "Estado", icono: LayoutDashboard, exacto: true }] },
  { id: "equipos", texto: "Equipos", icono: Monitor, pestanas: [{ href: "/todos/equipos", texto: "Equipos", icono: Monitor }] },
  { id: "copias", texto: "Copias", icono: FolderSync, pestanas: [{ href: "/todos/copias", texto: "Copias", icono: FolderSync }] },
  { id: "recuperar", texto: "Recuperar", icono: History, pestanas: [{ href: "/todos/restauraciones", texto: "Restauraciones", icono: ArchiveRestore }] },
  { id: "avisos", texto: "Avisos", icono: BellRing, pestanas: [{ href: "/todos/avisos", texto: "Avisos", icono: BellRing }] },
  {
    id: "actividad",
    texto: "Actividad",
    icono: Activity,
    pestanas: [
      { href: "/todos/ordenes", texto: "Órdenes en espera", icono: ClipboardList },
      { href: "/todos/en-curso", texto: "En curso", icono: Activity },
    ],
  },
];

/** ¿Está esta dirección en esta pestaña de «Todos los clientes»? */
export const enPestanaTodos = (p: { href: string; exacto?: boolean }, pathname: string) =>
  p.exacto ? pathname === p.href : pathname === p.href || pathname.startsWith(p.href + "/");
