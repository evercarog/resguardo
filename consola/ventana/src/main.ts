// La ventana del agente (docs/agente-ventana.md).
import "$ui/estilos.css";
import "./ventana.css";
import { mount } from "svelte";
import App from "./App.svelte";

// Tema forzado (solo en compilaciones de desarrollo del agente, para las capturas).
const tema = (window as { __resguardoTema?: string }).__resguardoTema;
if (tema) document.documentElement.dataset.theme = tema;

mount(App, { target: document.getElementById("app")! });
