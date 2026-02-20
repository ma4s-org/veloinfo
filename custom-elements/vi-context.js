let instance = null;

export const registerViMain = (el) => { instance = el; };
export const unregisterViMain = () => { instance = null; };
export const getViMain = () => instance;
