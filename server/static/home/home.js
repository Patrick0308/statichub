function emitAnalytics(eventName, payload = {}) {
  const detail = { event: eventName, ...payload };

  try {
    if (Array.isArray(window.dataLayer)) {
      window.dataLayer.push(detail);
    }
    window.dispatchEvent(new CustomEvent('statichub:analytics', { detail }));
  } catch (_err) {
    // no-op: analytics must never break homepage interactions
  }
}

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

        if (group.getAttribute('data-tabs') === 'quickstart') {
          if (target === 'skill') {
            emitAnalytics('switch_path_skill');
          } else if (target === 'cli') {
            emitAnalytics('switch_path_cli');
          }
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
        const os = button.getAttribute('data-os');
        if (os) {
          emitAnalytics('copy_install_command', { os });
        }
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

function setupAnalyticsCtas() {
  const installPrimary = document.querySelector('[data-analytics="install-primary"]');
  if (installPrimary) {
    installPrimary.addEventListener('click', () => {
      emitAnalytics('click_install_primary');
    });
  }

  const useSkill = document.querySelector('[data-analytics="use-skill"]');
  if (useSkill) {
    useSkill.addEventListener('click', () => {
      emitAnalytics('click_use_skill');
    });
  }
}

function setupQuickstartStepTracking() {
  const steps = document.querySelectorAll('[data-quickstart-step]');
  steps.forEach((step) => {
    step.addEventListener('click', () => {
      const stepIndex = Number(step.getAttribute('data-quickstart-step'));
      if (!Number.isNaN(stepIndex)) {
        emitAnalytics('complete_quickstart_step', { step_index: stepIndex });
      }
    });
  });
}

document.addEventListener('DOMContentLoaded', () => {
  setupTabs();
  setupCopyButtons();
  setupAnalyticsCtas();
  setupQuickstartStepTracking();
});
