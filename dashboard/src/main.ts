import { mount } from 'svelte';
import './app.css';
import './themes/default.css';
import './themes/nord.css';
import './themes/solarized.css';
import './themes/catppuccin.css';
import './themes/tokyo-night.css';
import './themes/ayu.css';
import './themes/night-owl.css';
import './themes/gruvbox.css';
import './themes/noctis.css';
import './themes/sublime-material.css';
import './themes/high-contrast.css';
import './themes/sticky-notes.css';
import './themes/hand-drawn.css';
import './themes/doc-styles.css';
import App from './App.svelte';

// Mount straight into <body> so the DOM matches the legacy dashboard (no wrapper element).
const app = mount(App, { target: document.body });

export default app;
