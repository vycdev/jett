function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function digit(ch: string): number {
    let pos: number = 0;
    while ((pos < 10)) {
        if ((at("0123456789", pos) == ch)) {
            return pos;
        }
        pos = (pos + 1);
    }
    return (-1);
}

export function uint_value(text: string, bound: number): number {
    if ((length(text) == 0)) {
        return (-1);
    }
    let value: number = 0;
    let pos: number = 0;
    while ((pos < length(text))) {
        let num: number = digit(at(text, pos));
        if ((num < 0)) {
            return (-1);
        }
        value = ((value * 10) + num);
        if ((value > bound)) {
            return (-1);
        }
        pos = (pos + 1);
    }
    return value;
}

export function component_end(text: string, start: number): number {
    let pos: number = start;
    while ((pos < length(text))) {
        if ((at(text, pos) == ".")) {
            return pos;
        }
        pos = (pos + 1);
    }
    return pos;
}

export function component_value(text: string, start: number, end: number): number {
    if ((start >= length(text))) {
        return 0;
    }
    return uint_value(part(text, start, end), 999);
}

export function solve(left: string, right: string): number {
    let lp: number = 0;
    let rp: number = 0;
    while (((lp < length(left)) || (rp < length(right)))) {
        let le: number = component_end(left, lp);
        let re: number = component_end(right, rp);
        let lv: number = component_value(left, lp, le);
        let rv: number = component_value(right, rp, re);
        if ((lv < rv)) {
            return (-1);
        }
        if ((lv > rv)) {
            return 1;
        }
        lp = (le + 1);
        rp = (re + 1);
    }
    return 0;
}
