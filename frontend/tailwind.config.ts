/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ['class'],
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        primary: {
          DEFAULT: '#171717',
          foreground: '#ffffff',
        },
        ink: '#171717',
        body: '#4d4d4d',
        mute: '#888888',
        hairline: {
          DEFAULT: '#ebebeb',
          strong: '#a1a1a1',
        },
        canvas: {
          DEFAULT: '#ffffff',
          soft: '#fafafa',
          'soft-2': '#f5f5f5',
        },
        link: {
          DEFAULT: '#0070f3',
          deep: '#0761d1',
          'bg-soft': '#d3e5ff',
        },
        success: '#0070f3',
        error: {
          DEFAULT: '#ee0000',
          soft: '#f7d4d6',
          deep: '#c50000',
        },
        warning: {
          DEFAULT: '#f5a623',
          soft: '#ffefcf',
          deep: '#ab570a',
        },
        violet: {
          DEFAULT: '#7928ca',
          soft: '#d8ccf1',
          deep: '#4c2889',
        },
        cyan: {
          DEFAULT: '#50e3c2',
          soft: '#aaffec',
          deep: '#29bc9b',
        },
        highlight: {
          pink: '#ff0080',
          magenta: '#eb367f',
        },
        gradient: {
          'develop-start': '#007cf0',
          'develop-end': '#00dfd8',
          'preview-start': '#7928ca',
          'preview-end': '#ff0080',
          'ship-start': '#ff4d4d',
          'ship-end': '#f9cb28',
        },
      },
      fontFamily: {
        sans: ['Geist', 'Inter', 'system-ui', '-apple-system', 'sans-serif'],
        mono: [
          'Geist Mono',
          'JetBrains Mono',
          'ui-monospace',
          'SFMono-Regular',
          'Menlo',
          'Monaco',
          'monospace',
        ],
      },
      fontSize: {
        'display-xl': [
          '48px',
          { lineHeight: '48px', fontWeight: '600', letterSpacing: '-2.4px' },
        ],
        'display-lg': [
          '32px',
          { lineHeight: '40px', fontWeight: '600', letterSpacing: '-1.28px' },
        ],
        'display-md': [
          '24px',
          { lineHeight: '32px', fontWeight: '600', letterSpacing: '-0.96px' },
        ],
        'display-sm': [
          '20px',
          { lineHeight: '28px', fontWeight: '600', letterSpacing: '-0.6px' },
        ],
        'body-lg': ['18px', { lineHeight: '28px', fontWeight: '400' }],
        'body-md': ['16px', { lineHeight: '24px', fontWeight: '400' }],
        'body-md-strong': ['16px', { lineHeight: '24px', fontWeight: '500' }],
        'body-sm': ['14px', { lineHeight: '20px', fontWeight: '400', letterSpacing: '-0.28px' }],
        'body-sm-strong': ['14px', { lineHeight: '20px', fontWeight: '500', letterSpacing: '-0.28px' }],
        caption: ['12px', { lineHeight: '16px', fontWeight: '400' }],
        'caption-mono': ['12px', { lineHeight: '16px', fontWeight: '400', fontFamily: 'Geist Mono, JetBrains Mono, ui-monospace, monospace' }],
        code: ['13px', { lineHeight: '20px', fontWeight: '400', fontFamily: 'Geist Mono, JetBrains Mono, ui-monospace, monospace' }],
        'button-md': ['14px', { lineHeight: '20px', fontWeight: '500' }],
        'button-lg': ['16px', { lineHeight: '24px', fontWeight: '500' }],
      },
      borderRadius: {
        none: '0px',
        xs: '4px',
        sm: '6px',
        md: '8px',
        lg: '12px',
        xl: '16px',
        'pill-sm': '64px',
        pill: '100px',
        full: '9999px',
      },
      spacing: {
        xxs: '4px',
        xs: '8px',
        sm: '12px',
        md: '16px',
        lg: '24px',
        xl: '32px',
        '2xl': '40px',
        '3xl': '48px',
        '4xl': '64px',
        '5xl': '96px',
        '6xl': '128px',
        section: '192px',
      },
      boxShadow: {
        'level-1': '0 0 0 1px #00000014',
        'level-2':
          '0px 1px 1px #00000005, 0px 2px 2px #0000000a, inset 0 0 0 1px #00000014',
        'level-3':
          '0px 2px 2px #0000000a, 0px 8px 8px -8px #0000000a, inset 0 0 0 1px #00000014',
        'level-4':
          '0px 2px 2px #0000000a, 0px 8px 16px -4px #0000000a, inset 0 0 0 1px #00000014',
        'level-5':
          '0px 1px 1px #00000005, 0px 8px 16px -4px #0000000a, 0px 24px 32px -8px #0000000f, inset 0 0 0 1px #00000014',
      },
      keyframes: {
        'fade-in': {
          '0%': { opacity: '0', transform: 'translateY(4px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
      },
      animation: {
        'fade-in': 'fade-in 0.3s ease-out',
      },
    },
  },
  plugins: [require('tailwindcss-animate')],
};
