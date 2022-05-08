#!/usr/bin/python

import matplotlib.pyplot as plt

dates = ['10/1/21', '10/14/21', '10/17/21']
tests = {
    'passed': [121, 126, 160],
    'skipped': [85, 85, 85],
    'failed': [45, 40, 11],
}
colors = [
    'green',
    'blue',
    'red',
]

fig, ax = plt.subplots()
ax.stackplot(dates, tests.values(), labels=tests.keys(), colors=colors)
ax.legend(loc='upper left')
ax.set_title('AOT runtime test status')
ax.set_xlabel('Dates')
ax.set_ylabel('Tests')

plt.savefig('tests_plot.png')
