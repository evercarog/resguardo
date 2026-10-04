// La ventana del agente (docs/agente-ventana.md).
import "$ui/estilos.css";
import "./ventana.css";
import { mount } from "svelte";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
