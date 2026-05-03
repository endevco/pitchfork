// Usage:
//   bun run ansi_output.ts <color> <text>
//   e.g. bun run ansi_output.ts 32 green   -> prints "green" in green ANSI color
//        bun run ansi_output.ts 31 red      -> prints "red" in red ANSI color
//        bun run ansi_output.ts 34 blue     -> prints "blue" in blue ANSI color

const color = process.argv[2] ?? "32";
const text = process.argv[3] ?? "hello";
console.log(`\x1b[${color}m${text}\x1b[0m`);
