import { caller } from './main';
export function View() { return <div>{caller()}</div>; }
