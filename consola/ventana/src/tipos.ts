// Lo que escribe el servicio para la bandeja y la ventana (crates/agente/src/bandeja.rs y escritorio.rs).

export type Ventana = "off" | "siempre_disponible" | "al_trabajar";
export type Avisos = "off" | "errores" | "todo";
export type TipoActividad = "copia" | "restauracion" | "verificacion" | "copia_externa" | "espejo" | "nube";

export interface Escritorio {
  ventana: Ventana;
  avisos: Avisos;
}

export interface CopiaBandeja {
  clave: string;
  nombre: string;
  resultado: "ok" | "warning" | "error" | "";
  cuando?: string | null;
  proxima?: string | null;
  pausada?: boolean;
}

export interface Actividad {
  id: string;
  clave: string;
  tipo: TipoActividad;
  nombre: string;
  fase: string;
  porcentaje?: number;
  archivos?: number;
  archivos_total?: number;
  bytes?: number;
  bytes_total?: number;
  velocidad?: number;
  lectura?: number;
  subida?: number;
  archivos_s?: number;
  quedan_s?: number;
  empezo: string;
}

export interface Hecha {
  clave: string;
  tipo: TipoActividad;
  nombre: string;
  resultado: "ok" | "warning" | "error";
  cuando: string;
  bytes?: number;
}

export interface EstadoBandeja {
  text: string;
  privacy: string;
  show: boolean;
  vinculado: boolean;
  aviso?: string | null;
  consola?: string | null;
  copias: CopiaBandeja[];
  pedir: boolean;
  escrito?: string | null;
  escritorio: Escritorio;
  actividades: Actividad[];
  hechas: Hecha[];
  local: boolean;
}

export interface Dia {
  dia: string;
  ok: number;
  aviso: number;
  fallo: number;
}

/** `[segundo Unix, lectura B/s, escritura B/s, archivos/s, tipo]`. */
export type Punto = [number, number, number, number, TipoActividad];

export interface EstadoVentana {
  v: number;
  escrito?: string | null;
  serie: Punto[];
  historial: Dia[];
}

export interface Datos {
  bandeja: EstadoBandeja | null;
  ventana: EstadoVentana | null;
  ahora: string;
}

/** Lo que dice el servicio de este equipo (`hola`). */
export type Modo = "sin_clave" | "local" | "gestionado" | "web" | "pendiente" | "desvinculado";
