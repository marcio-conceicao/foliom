import { mount } from 'svelte';
import App from './App.svelte';
import './styles/global.css';
import './styles/sidebar.css';
import './styles/blocks.css';
import './styles/prism-foliom.css';
import './styles/palette.css';
import { installZoomListener } from './lib/zoom';

installZoomListener();

const target = document.getElementById('app');
if (!target) {
  throw new Error('Foliom: #app mount target not found in index.html');
}

const app = mount(App, { target });

export default app;
