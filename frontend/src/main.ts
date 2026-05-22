import { mount } from 'svelte';
import App from './App.svelte';
import './styles/global.css';

const target = document.getElementById('app');
if (!target) {
  throw new Error('Foliom: #app mount target not found in index.html');
}

const app = mount(App, { target });

export default app;
