// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Pure utility helpers for OutputSection.svelte.
 *
 * Extracted to keep the component focused on UI orchestration. These
 * functions have no side effects and no Svelte store dependencies.
 */

/** Truncate long directory paths for display */
export function truncatePath(path: string, maxLen = 30): string {
  if (path.length <= maxLen) return path;
  const parts = path.replace(/\\/g, '/').split('/');
  if (parts.length <= 2) return '...' + path.slice(-maxLen);
  return parts[0] + '/.../' + parts[parts.length - 1];
}

/** Prettify a JSON template string that may contain {{placeholder}} tokens */
export function prettifyJSONTemplate(template: string): string {
  if (!template) return '';
  try {
    const testJSON = template
      .replace(/"?\{\{id\}\}"?/g, '"___PLACEHOLDER_ID___"')
      .replace(/"?\{\{text\}\}"?/g, '"___PLACEHOLDER_TEXT___"')
      .replace(/"?\{\{timestamp\}\}"?/g, '"___PLACEHOLDER_TIMESTAMP___"')
      .replace(/"?\{\{language\}\}"?/g, '"___PLACEHOLDER_LANGUAGE___"')
      .replace(/"?\{\{model\}\}"?/g, '"___PLACEHOLDER_MODEL___"')
      .replace(/\{\{duration\}\}/g, '999999');
    const parsed = JSON.parse(testJSON);
    let prettified = JSON.stringify(parsed, null, 2);
    prettified = prettified
      .replace(/"___PLACEHOLDER_ID___"/g, '"{{id}}"')
      .replace(/"___PLACEHOLDER_TEXT___"/g, '"{{text}}"')
      .replace(/"___PLACEHOLDER_TIMESTAMP___"/g, '"{{timestamp}}"')
      .replace(/"___PLACEHOLDER_LANGUAGE___"/g, '"{{language}}"')
      .replace(/"___PLACEHOLDER_MODEL___"/g, '"{{model}}"')
      .replace(/999999/g, '{{duration}}');
    return prettified;
  } catch {
    return template;
  }
}

/** Validate that a JSON metadata template is syntactically correct */
export function validateJSONTemplate(template: string): boolean {
  try {
    const testJSON = template
      .replace(/"?\{\{id\}\}"?/g, '"test-id"')
      .replace(/"?\{\{text\}\}"?/g, '"test-text"')
      .replace(/"?\{\{timestamp\}\}"?/g, '"2025-01-01T00:00:00.000Z"')
      .replace(/"?\{\{language\}\}"?/g, '"en"')
      .replace(/"?\{\{model\}\}"?/g, '"test-model"')
      .replace(/\{\{duration\}\}/g, '123');
    JSON.parse(testJSON);
    return true;
  } catch {
    return false;
  }
}

/** Render a preview of the template filled with mock data */
export function generateTemplatePreview(template: string): string {
  const mockData = {
    id: '1765193114412',
    text: 'Test one, two.',
    timestamp: '2025-12-08T11:25:14.412Z',
    language: 'en',
    model: 'faster-whisper-large-v3-turbo',
    duration: '2',
  };
  return template
    .replace(/\{\{id\}\}/g, mockData.id)
    .replace(/\{\{text\}\}/g, mockData.text)
    .replace(/\{\{timestamp\}\}/g, mockData.timestamp)
    .replace(/\{\{language\}\}/g, mockData.language)
    .replace(/\{\{model\}\}/g, mockData.model)
    .replace(/\{\{duration\}\}/g, mockData.duration);
}

/** Apply syntax highlighting to a JSON string (returns HTML) */
export function highlightJSON(json: string): string {
  // The result is rendered with {@html}, so escape markup characters first.
  // Quotes stay as-is: they're harmless in element content and the regexes
  // below key off them.
  let highlighted = json.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  highlighted = highlighted.replace(/\{\{(\w+)\}\}/g, '\u0000TEMPLATE_$1\u0000');
  highlighted = highlighted
    .replace(/("(?:[^"\\]|\\.)*")\s*:/g, '<span class="json-key">$1</span>:')
    .replace(/:\s*("(?:[^"\\]|\\.)*")/g, ': <span class="json-string">$1</span>')
    .replace(/:\s*(\d+)/g, ': <span class="json-number">$1</span>')
    .replace(/:\s*(true|false|null)/g, ': <span class="json-boolean">$1</span>')
    .replace(/([{}[\],])/g, '<span class="json-punctuation">$1</span>');
  highlighted = highlighted.replace(
    /\u0000TEMPLATE_(\w+)\u0000/g,
    '<span class="json-template">{<!-- -->{$1}<!-- -->}</span>'
  );
  return highlighted;
}
