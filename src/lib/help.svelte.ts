// Centro de ayuda: estado global para abrirlo desde cualquier parte (F1, la
// barra lateral o los enlaces «?» junto a los campos).

export const help = $state<{ open: boolean; topic: string | null }>({ open: false, topic: null });

/** Abre la ayuda; con `topic` (id de una sección o de un apartado) va directamente allí. */
export function openHelp(topic: string | null = null) {
  help.topic = topic;
  help.open = true;
}

export function closeHelp() {
  help.open = false;
  help.topic = null;
}
