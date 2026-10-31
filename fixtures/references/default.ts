import { default as run, type Options as Settings } from "./service";
const execute = () => run();
export default execute;
export type { Settings as Options };
export type { Shape } from "./types";
