// Runs before first paint (inlined in <head>) so the page never flashes the wrong theme.
// Stored choice wins; otherwise follow the OS; with no OS preference, default to dark.
export const THEME_STORAGE_KEY = 'kivori-theme';

export const THEME_INIT_SCRIPT = `(function(){try{var t=null;try{t=localStorage.getItem('${THEME_STORAGE_KEY}')}catch(e){}
if(t!=='light'&&t!=='dark'){t=window.matchMedia('(prefers-color-scheme: light)').matches?'light':'dark'}
document.documentElement.classList.toggle('dark',t==='dark');document.documentElement.style.colorScheme=t}catch(e){}})();`;
