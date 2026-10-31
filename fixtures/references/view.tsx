import Widget from "./Widget";
import { load } from "./data";
export function View() { return <Widget value={load()} />; }
