// Runs before first paint (inlined in <head>) so the page never flashes the wrong theme.
// Light by default; only a stored choice (the theme toggle) switches to dark.
export const THEME_STORAGE_KEY = 'kivori-theme';

export const THEME_INIT_SCRIPT = `(function(){try{var t=null;try{t=localStorage.getItem('${THEME_STORAGE_KEY}')}catch(e){}
if(t!=='dark'){t='light'}
document.documentElement.classList.toggle('dark',t==='dark');document.documentElement.style.colorScheme=t}catch(e){}})();`;
