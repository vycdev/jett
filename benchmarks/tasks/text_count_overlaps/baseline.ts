function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function solve(text: string, needle: string): number {
    let count: number = 0;
    let pos: number = 0;
    while (((pos + length(needle)) <= length(text))) {
        if ((part(text, pos, (pos + length(needle))) == needle)) {
            count = (count + 1);
        }
        pos = (pos + 1);
    }
    return count;
}
