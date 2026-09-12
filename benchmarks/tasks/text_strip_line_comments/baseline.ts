function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function quote_mode(mode: number, ch: string): number {
    if ((mode == 2)) {
        return 1;
    }
    if (((mode == 1) && (ch == "\\"))) {
        return 2;
    }
    if ((ch == "\"")) {
        return (1 - mode);
    }
    return mode;
}

export function solve(text: string): string {
    let out: string = "";
    let mode: number = 0;
    let comment: boolean = false;
    let pos: number = 0;
    while ((pos < length(text))) {
        let ch: string = at(text, pos);
        if (comment) {
            if ((ch == "\n")) {
                comment = false;
                out = (out + ch);
            }
        }
        else {
            if (((mode == 0) && (ch == "#"))) {
                comment = true;
            }
            else {
                out = (out + ch);
                mode = quote_mode(mode, ch);
            }
        }
        pos = (pos + 1);
    }
    return out;
}
