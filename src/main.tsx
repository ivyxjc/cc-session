import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";
import { setLocale } from "./lib/format";
import { applyFonts } from "./lib/fonts";

// Restore settings from localStorage
const savedLocale = localStorage.getItem("locale");
if (savedLocale) setLocale(savedLocale);
applyFonts();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
