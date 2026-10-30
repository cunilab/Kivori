import { createRoot } from 'react-dom/client';
import { VirtualDevice } from './features/virtual-device/VirtualDevice';
import './index.css';

const root = document.getElementById('root');
if (root && import.meta.env.DEV) {
  root.dataset.developmentSurface = 'kivori-virtual-device-must-not-ship';
  createRoot(root).render(<VirtualDevice />);
}
