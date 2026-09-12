function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function solve(text: string, pattern: string): boolean {
    let ti: number = 0;
    let pi: number = 0;
    let star: number = (-1);
    let retry: number = 0;
    while ((ti < length(text))) {
        let ch: string = at(pattern, pi);
        if (((pi < length(pattern)) && ((ch == "?") || (ch == at(text, ti))))) {
            ti = (ti + 1);
            pi = (pi + 1);
        }
        else {
            if ((ch == "*")) {
                star = pi;
                retry = ti;
                pi = (pi + 1);
            }
            else {
                if ((star < 0)) {
                    return false;
                }
                retry = (retry + 1);
                ti = retry;
                pi = (star + 1);
            }
        }
    }
    while ((at(pattern, pi) == "*")) {
        pi = (pi + 1);
    }
    return (pi == length(pattern));
}
