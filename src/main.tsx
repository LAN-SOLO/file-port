import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles.css';
import { readTheme } from './theme';

// Theme vor dem ersten Render auf <html> setzen — sonst blitzt Dark kurz auf.
document.documentElement.dataset.theme = readTheme();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
