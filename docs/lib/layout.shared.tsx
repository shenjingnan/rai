import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: 'ZapMomo 文档',
    },
    // TODO: 仓库由 RAI 重命名为 ZapMomo 后更新为新 remote URL
    githubUrl: 'https://github.com/shenjingnan/rai',
    links: [
      {
        text: 'GitHub',
        url: 'https://github.com/shenjingnan/rai',
      },
    ],
  };
}
