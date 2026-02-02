/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      keyframes: {
        'music-bar': {
          '0%, 100%': { height: '4px' },
          '50%': { height: '16px' },
        }
      },
      animation: {
        'music-bar-1': 'music-bar 0.8s ease-in-out infinite',
        'music-bar-2': 'music-bar 1.0s ease-in-out infinite 0.2s',
        'music-bar-3': 'music-bar 0.6s ease-in-out infinite 0.4s',
      },
      colors: {
        primary: {
          50: '#f0f9ff',
          100: '#e0f2fe',
          200: '#bae6fd',
          300: '#7dd3fc',
          400: '#38bdf8',
          500: '#0ea5e9',
          600: '#0284c7',
          700: '#0369a1',
          800: '#075985',
          900: '#0c4a6e',
          950: '#082f49',
        },
      },
    },
  },
  plugins: [],
}
