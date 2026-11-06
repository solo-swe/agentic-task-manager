import { fireEvent, render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ProjectLogo from '../../src/lib/ui/ProjectLogo.svelte';

describe('ProjectLogo', () => {
  it('shows the logo in a box of the given size', () => {
    const { container } = render(ProjectLogo, {
      project: { id: 'abcd1234', name: 'Agentic Task Manager', logo: 'p/abcd1234/logo' },
      size: 24,
    });
    const box = container.querySelector('.project-logo')!;
    expect(box).not.toHaveClass('monogram');
    expect(box).not.toHaveAttribute('data-initials');
    expect(box).toHaveStyle('width: 24px; height: 24px');
    expect(box.querySelector('img')).toHaveAttribute('src', 'p/abcd1234/logo');
    expect(box.querySelector('img')).toHaveAttribute('width', '24');
  });

  it('without a logo: the monogram, coloured by the id', () => {
    const { container } = render(ProjectLogo, { project: { id: 'abcd1234', name: 'Agentic Task Manager' } });
    const box = container.querySelector('.project-logo')!;
    expect(box).toHaveClass('monogram');
    expect(box).toHaveAttribute('data-initials', 'AT');
    expect(box.textContent?.trim()).toBe('');
    expect(box.querySelector('img')).toBeNull();
    expect((box as HTMLElement).style.getPropertyValue('--monogram-color')).toMatch(/^var\(--/);
    expect(box).toHaveStyle('width: 16px');
  });

  it('a logo that fails to load gives way to the monogram', async () => {
    const { container } = render(ProjectLogo, { project: { id: 'x', name: 'demo', logo: 'p/x/logo' } });
    await fireEvent.error(container.querySelector('img')!);
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelector('.project-logo')).toHaveAttribute('data-initials', 'DE');
  });
});
