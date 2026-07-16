import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    rules: {
      // React Compiler-era diagnostics; compiler not in use, existing
      // fetch-on-mount effects are stable. Warn until compiler adoption.
      'react-hooks/set-state-in-effect': 'warn',
      // Stale-closure detector — kept at error; intentional omissions get
      // inline suppressions with a reason.
      'react-hooks/exhaustive-deps': 'error',
      // Underscore prefix marks intentionally unused (e.g. destructured
      // props kept for the StepProps signature).
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' },
      ],
    },
  },
  {
    // Context provider modules export a hook alongside the provider;
    // only affects dev HMR granularity, not production.
    files: ['src/hooks/*.tsx'],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
])
