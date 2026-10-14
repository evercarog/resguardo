// Tipos de las pruebas funcionales (plan 0.8.0, E3, 14k) y de las pruebas de
// restauración desde un espejo (E4, 14j). Aparte de tipos.ts para no mezclarlos.

export type MotivoPrueba = "programada" | "auditoria" | "cambio" | "incidente" | "otro";
export type ResultadoPrueba = "ok" | "parcial" | "fallo";
export type DesdePrueba = "repositorio" | "espejo" | "derivada" | "otro";

export interface RepoRef {
  equipo: string;
  repo: string;
}

/** Lo que se manda al registrar o cambiar una prueba funcional. */
export interface EntradaPruebaFuncional {
  /** AAAA-MM-DD. */
  fecha: string;
  motivo: MotivoPrueba;
  /** `cliente` (todo) o `repos` (los de la lista). */
  alcance: "cliente" | "repos";
  repos: RepoRef[];
  que: string;
  donde: string;
  desde?: { tipo: DesdePrueba; destino?: string | null; nombre?: string | null } | null;
  /** Vacío: quien la registra. */
  quien: string;
  resultado: ResultadoPrueba;
  incidencias?: string | null;
  /** Minutos hasta tener los datos de vuelta (el RTO real). */
  rto_min?: number | null;
  /** Antigüedad de lo recuperado, en minutos (el RPO). */
  rpo_min?: number | null;
  restauracion?: { equipo: string; id: string } | null;
  notas?: string | null;
}

export interface AdjuntoPrueba {
  n: number;
  nombre: string;
  tipo: "application/pdf" | "image/png" | "image/jpeg";
  tamano: number;
}

/** Una prueba funcional guardada. */
export interface PruebaFuncional extends EntradaPruebaFuncional {
  id: string;
  creada: string;
  actualizada: string;
  por: string;
  adjuntos: AdjuntoPrueba[];
  /** Si cumple los objetivos de la política (null: sin objetivo o sin el dato). */
  cumple_rto?: boolean | null;
  cumple_rpo?: boolean | null;
}

export interface PlazoPrueba {
  ultima: string | null;
  vence: string;
  fase: "al_dia" | "pronto" | "vencida";
  meses: number;
}

export interface PoliticaPruebas {
  activo: boolean;
  meses: number;
  aviso_dias: number;
  rto_objetivo_min?: number | null;
  rpo_objetivo_min?: number | null;
  repos: (RepoRef & { meses: number })[];
}

export interface PruebasFuncionales {
  ajustes: PoliticaPruebas & { actualizada?: string | null; por?: string | null };
  /** null: la política está apagada. */
  estado: { cliente: PlazoPrueba; repos: (RepoRef & { plazo: PlazoPrueba })[] } | null;
  pruebas: PruebaFuncional[];
  limites?: { adjunto: number; adjuntos: number };
}

/** E4 (14j): una prueba de restauración desde un espejo, como la cuenta el equipo en su informe. */
export interface PruebaEspejoInforme {
  id: string;
  nombre: string;
  /** La clave del destino en la consola (la que se mandó). */
  destino?: string | null;
  /** `espejo_equipo`, `nube` o `carpeta` (las programadas). */
  forma?: string | null;
  trabajo?: string | null;
  nube?: string | null;
  cada_dias?: number | null;
  proxima?: string | null;
  ultima?: string | null;
  inicio?: string | null;
  resultado?: "ok" | "fallo" | null;
  mensaje_corto?: string | null;
  archivos?: number | null;
  bytes?: number | null;
  version?: string | null;
}

/** Lo que se manda al equipo en `probar_restauracion { espejo | programar_espejo }`. */
export interface PruebaEspejoOrden {
  id: string;
  nombre: string;
  destino?: string | null;
  trabajo?: string;
  nube?: string;
  carpeta?: string;
  cada_dias?: number;
}
