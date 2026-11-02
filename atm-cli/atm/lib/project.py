"""A project as atm-lib sees it: where its data is kept, its code's version control, and who acts."""


class Project:
    def __init__(self, store, vcs=None, actor=None):
        self.store = store
        self.vcs = vcs
        self.actor = actor
