// 15nn: aviso de nombre repetido al crear o recibir un cliente. Se compara sin
// distinguir mayúsculas, tildes ni espacios de más («Café del Sur» = «cafe  del sur»).

/** Lo que basta de un cliente para avisar (de «Clientes del servidor» o de los de la cuenta). */
export interface ClienteConNombre {
  id: string;
  nombre: string;
  /** Archivado en este servidor (15nn). */
  archivado?: string | null;
  /** ¿Es miembro quien crea? (sin el campo: sí, es de su lista). */
  soy_miembro?: boolean;
}

/** El nombre tal como se compara. */
export function normalizarNombre(nombre: string): string {
  return nombre.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/\s+/g, " ").trim();
}

/** El cliente que ya se llama así (el activo primero), o null. */
export function clienteRepetido<C extends ClienteConNombre>(nombre: string, clientes: readonly C[]): C | null {
  const n = normalizarNombre(nombre);
  if (!n) return null;
  const iguales = clientes.filter((c) => normalizarNombre(c.nombre) === n);
  return iguales.find((c) => !c.archivado) ?? iguales[0] ?? null;
}
