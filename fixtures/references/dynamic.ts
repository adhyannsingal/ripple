import * as ns from "./auth";
export function uncertain(obj: any, key: string) {
    ns.login();
    obj[key]();
    obj?.run();
    import("./auth");
    import(key);
    new ns.Service();
    eval(key);
    require("./auth");
}
