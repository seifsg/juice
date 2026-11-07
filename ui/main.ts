import { invoke } from '@tauri-apps/api/core';

const app = document.querySelector<HTMLElement>('#app');
if (!app) throw new Error('juice: #app missing');
app.innerHTML = '<h1>Juice</h1><p>spike</p>';
await invoke('fit', { height: document.documentElement.scrollHeight });
