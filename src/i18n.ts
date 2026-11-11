import { useEffect, useState, useCallback } from 'react';
import { LANG_STORAGE_KEY, PanelLanguage } from './i18n/dictionaries';
import { applyDomLanguageTranslation } from './i18n/domTranslator';

export * from './i18n/dictionaries';
export * from './i18n/domTranslator';

export function usePanelLanguage() {
  const [lang, setLangState] = useState<PanelLanguage>(() => {
    const saved = localStorage.getItem(LANG_STORAGE_KEY);
    return saved === 'en' || saved === 'vi' ? saved : 'vi';
  });

  const setLang = useCallback((next: PanelLanguage) => {
    localStorage.setItem(LANG_STORAGE_KEY, next);
    setLangState(next);
  }, []);

  const toggleLang = useCallback(() => {
    setLangState((prev) => {
      const next = prev === 'vi' ? 'en' : 'vi';
      localStorage.setItem(LANG_STORAGE_KEY, next);
      return next;
    });
  }, []);

  const tr = useCallback(
    (viText: string, enText: string): string => {
      return lang === 'en' ? enText : viText;
    },
    [lang]
  );

  useEffect(() => {
    document.documentElement.lang = lang;
    applyDomLanguageTranslation(document.body, lang);

    let rafId: number | null = null;
    const observer = new MutationObserver(() => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        applyDomLanguageTranslation(document.body, lang);
      });
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true
    });

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      observer.disconnect();
    };
  }, [lang]);

  return { lang, setLang, toggleLang, tr };
}
