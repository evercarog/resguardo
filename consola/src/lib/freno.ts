// Freno para las cargas de fondo de una pantalla: como mucho una cada `minEntre`
// ms y nunca dos a la vez. Lo que se pide mientras tanto se junta en UNA carga
// más, al final. Así, aunque lleguen cientos de avisos seguidos por el canal en
// vivo (un equipo que se conecta y se cae en bucle, muchas órdenes), cada
// pantalla pide como mucho 60 / (minEntre / 1000) veces por minuto.
// scripts/vectores-emparejar.ts lo comprueba.

export interface Freno {
  /** Pide una carga (ya, o en cuanto toque). */
  pedir: () => void;
  /** Cuándo empezó la última carga (Date.now()). */
  ultima: () => number;
  /** No más cargas (al salir de la pantalla). */
  parar: () => void;
}

export function frenar(cargar: () => unknown, minEntre: number, ahora: () => number = Date.now): Freno {
  let ultima = ahora();
  let enCurso = false;
  let otraVez = false;
  let diferida: ReturnType<typeof setTimeout> | null = null;
  let parado = false;
  const pedir = () => {
    if (parado) return;
    if (enCurso) {
      otraVez = true;
      return;
    }
    const falta = ultima + minEntre - ahora();
    if (falta > 0) {
      diferida ??= setTimeout(() => {
        diferida = null;
        pedir();
      }, falta);
      return;
    }
    ultima = ahora();
    enCurso = true;
    void Promise.resolve()
      .then(cargar)
      .catch(() => {})
      .finally(() => {
        enCurso = false;
        if (otraVez) {
          otraVez = false;
          pedir();
        }
      });
  };
  return {
    pedir,
    ultima: () => ultima,
    parar: () => {
      parado = true;
      if (diferida) clearTimeout(diferida);
    },
  };
}
