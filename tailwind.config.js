/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./dist/**/*.js",
    "./dist/**/*.html",
    "./_site/**/*.js",
    "./_site/**/*.html",
    "./demo/**/*.tp",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
