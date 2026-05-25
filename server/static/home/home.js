function setupTabs() {
  const groups = document.querySelectorAll('[data-tabs]');

  groups.forEach((group) => {
    const buttons = group.querySelectorAll('.tab-btn');
    const panels = group.querySelectorAll('[data-tab-panel]');

    buttons.forEach((button) => {
      button.addEventListener('click', () => {
        const target = button.getAttribute('data-tab-target');

        buttons.forEach((btn) => {
          btn.classList.remove('is-active');
          btn.setAttribute('aria-selected', 'false');
        });

        panels.forEach((panel) => {
          panel.classList.remove('is-active');
        });

        button.classList.add('is-active');
        button.setAttribute('aria-selected', 'true');

        const nextPanel = group.querySelector(`[data-tab-panel="${target}"]`);
        if (nextPanel) {
          nextPanel.classList.add('is-active');
        }
      });
    });
  });
}

async function copyText(text) {
  if (navigator.clipboard && window.isSecureContext) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const temp = document.createElement('textarea');
  temp.value = text;
  document.body.appendChild(temp);
  temp.select();
  document.execCommand('copy');
  document.body.removeChild(temp);
}

function selectCommandText(element) {
  if (!element) {
    return;
  }

  const selection = window.getSelection();
  if (!selection) {
    return;
  }

  const range = document.createRange();
  range.selectNodeContents(element);
  selection.removeAllRanges();
  selection.addRange(range);
}

function setupCopyButtons() {
  const buttons = document.querySelectorAll('.copy-btn[data-copy-target]');

  buttons.forEach((button) => {
    button.addEventListener('click', async () => {
      const targetId = button.getAttribute('data-copy-target');
      const source = targetId ? document.getElementById(targetId) : null;
      if (!source) {
        return;
      }

      try {
        await copyText(source.textContent.trim());
        const originalLabel = button.textContent;
        button.textContent = 'Copied';
        setTimeout(() => {
          button.textContent = originalLabel;
        }, 1200);
      } catch (_err) {
        button.textContent = 'Copy manually';
        selectCommandText(source);
      }
    });
  });
}

document.addEventListener('DOMContentLoaded', () => {
  setupTabs();
  setupCopyButtons();
});
