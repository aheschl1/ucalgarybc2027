"""UCalgary Battlecode. A bot imports ``ucbc.games.<game>``, the typed handle its ``step``
receives (``ucbc.handle`` is the game-agnostic part). The rest runs matches:
``ucbc.runner``, the ``ucbc`` command (``ucbc.cli``), and ``ucbc._guest``, what runs inside
each bot's interpreter, around the engine in ``ucbc._engine``."""
