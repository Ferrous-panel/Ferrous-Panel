import {
  PanelLanguage,
  VI_TO_EN_EXACT,
  VI_TO_EN_PHRASES
} from './dictionaries';

export function translateStringToEnglish(input: string): string {
  if (!input) return input;
  const trimmed = input.trim();
  if (!trimmed) return input;

  if (VI_TO_EN_EXACT[trimmed]) {
    return input.replace(trimmed, VI_TO_EN_EXACT[trimmed]);
  }

  let out = input;
  for (const [viPhrase, enPhrase] of VI_TO_EN_PHRASES) {
    if (out.includes(viPhrase)) {
      out = out.split(viPhrase).join(enPhrase);
    }
  }
  return out;
}

interface TrackedTextNode extends Text {
  __fpViText?: string;
  __fpEnText?: string;
}

interface TrackedElement extends HTMLElement {
  __fpViPlaceholder?: string;
  __fpEnPlaceholder?: string;
  __fpViTitle?: string;
  __fpEnTitle?: string;
}

/**
 * Traverses the DOM subtree and translates static Vietnamese text nodes to English
 * when `lang === 'en'`, or restores them to Vietnamese when `lang === 'vi'`.
 */
export function applyDomLanguageTranslation(
  root: HTMLElement,
  lang: PanelLanguage
): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;
      if (parent.closest('[data-fp-lang-ignore="true"]')) {
        return NodeFilter.FILTER_REJECT;
      }
      const tag = parent.tagName;
      if (
        tag === 'SCRIPT' ||
        tag === 'STYLE' ||
        tag === 'CODE' ||
        tag === 'PRE' ||
        tag === 'TEXTAREA'
      ) {
        return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    }
  });

  let currentNode = walker.nextNode() as TrackedTextNode | null;
  while (currentNode) {
    const val = currentNode.nodeValue || '';
    if (lang === 'en') {
      const candidateVi =
        currentNode.__fpViText !== undefined && val === currentNode.__fpViText
          ? currentNode.__fpViText
          : val;
      const translated = translateStringToEnglish(candidateVi);
      if (translated !== candidateVi) {
        currentNode.__fpViText = candidateVi;
        currentNode.__fpEnText = translated;
        if (currentNode.nodeValue !== translated) {
          currentNode.nodeValue = translated;
        }
      }
    } else if (lang === 'vi') {
      if (
        currentNode.__fpViText !== undefined &&
        currentNode.nodeValue === currentNode.__fpEnText
      ) {
        currentNode.nodeValue = currentNode.__fpViText;
      }
    }
    currentNode = walker.nextNode() as TrackedTextNode | null;
  }

  const elements = root.querySelectorAll<TrackedElement>(
    '[placeholder], [title]'
  );
  elements.forEach((el) => {
    if (el.closest('[data-fp-lang-ignore="true"]')) return;

    const placeholder = el.getAttribute('placeholder');
    if (placeholder !== null) {
      if (lang === 'en') {
        const candidateVi =
          el.__fpViPlaceholder !== undefined &&
          placeholder === el.__fpViPlaceholder
            ? el.__fpViPlaceholder
            : placeholder;
        const nextPh = translateStringToEnglish(candidateVi);
        if (nextPh !== candidateVi) {
          el.__fpViPlaceholder = candidateVi;
          el.__fpEnPlaceholder = nextPh;
          if (placeholder !== nextPh) {
            el.setAttribute('placeholder', nextPh);
          }
        }
      } else if (
        lang === 'vi' &&
        el.__fpViPlaceholder !== undefined &&
        placeholder === el.__fpEnPlaceholder
      ) {
        el.setAttribute('placeholder', el.__fpViPlaceholder);
      }
    }

    const title = el.getAttribute('title');
    if (title !== null) {
      if (lang === 'en') {
        const candidateVi =
          el.__fpViTitle !== undefined && title === el.__fpViTitle
            ? el.__fpViTitle
            : title;
        const nextTitle = translateStringToEnglish(candidateVi);
        if (nextTitle !== candidateVi) {
          el.__fpViTitle = candidateVi;
          el.__fpEnTitle = nextTitle;
          if (title !== nextTitle) {
            el.setAttribute('title', nextTitle);
          }
        }
      } else if (
        lang === 'vi' &&
        el.__fpViTitle !== undefined &&
        title === el.__fpEnTitle
      ) {
        el.setAttribute('title', el.__fpViTitle);
      }
    }
  });
}
