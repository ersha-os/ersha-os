import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx,elm}"
  ],
  theme: {
    extend: {},
  },
  plugins: [],
};

export default config;
