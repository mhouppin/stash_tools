#!/usr/bin/env python3

import argparse
import chess
import chess.engine
import chess.pgn
import chess.syzygy
import platform
import random

PATH_SEPARATOR = ";" if platform.system() == "Windows" else ":"

def maybe_analyse_board(engine, tb, board, sample_file, miseval_file, test_id, args):
    # Don't analyse checkmate/stalemate positions.
    if board.is_game_over():
        return

    dtz = tb.get_dtz(board)

    # Don't analyse positions with more chessmen than what the TBs support.
    if dtz == None:
        return

    result = "=" if abs(dtz) > 100 or dtz == 0 else "+" if dtz > 0 else "-"
    last_eval = None

    # Write the position to the sample file. This way we can later analyse it directly
    # with --epd=samples.epd to see how much we improved in endgames.
    sample_file.write(board.epd(hmvc=board.halfmove_clock, fmvn=board.fullmove_number) + '\n')
    sample_file.flush()

    # Add some logging.
    print("(%s) Analyzing %44s (%s)... " % (test_id, "'" + board.fen() + "'", result), end='', flush=True)

    # Perform several search iterations with reduced node counts to avoid wasting CPU time
    # on trivial draws/wins.
    for n in range(args.iters):
        # Perform a copy of the board so that python-chess sends "ucinewgame" to the engine
        # before requesting the analysis.
        board_copy = board.copy()

        # Compute the number of nodes to use for this iteration.
        p = n / (args.iters - 1)
        x = args.max_nodes / args.min_nodes
        cur_nodes = round(args.min_nodes * (x ** p))

        # Note that we do a copy of the board for each iteration so that python-chess sends
        # "ucinewgame" to the engine before launching the search.
        info = engine.analyse(board_copy, chess.engine.Limit(nodes=cur_nodes), game=board_copy)

        # Small safety check in case the engine failed to report a score.
        if not "score" in info:
            print("Error", flush=True)
            return

        else:
            score_cp = info["score"].relative.score(mate_score=40000)
            last_eval = score_cp

            # Check if we are correctly evaluating the position.
            if (result == "=" and abs(score_cp) < args.draw) \
                or (result == "+" and score_cp > +args.win) \
                or (result == "-" and score_cp < -args.win):

                # Add some logging.
                print("  Solved (%7d nodes, score %+7.2f)" % (cur_nodes, score_cp / 100.0),
                    flush=True)
                return

    miseval_type=""

    # Assign the type of miseval that we will write in the EPD entry
    if result == "=":
        if score_cp > args.draw:
            miseval_type = "Optimistic draw eval"
        else:
            miseval_type = "Pessimistic draw eval"
    elif result == "+":
        miseval_type = "Blind to win"
    else:
        miseval_type = "Blind to loss"

    # Write the position to the miseval EPD
    miseval_file.write(board.epd(
        hmvc=board.halfmove_clock,
        fmvn=board.fullmove_number,
        c0=miseval_type,
        ce=last_eval
        ) + '\n')
    miseval_file.flush()

    # Some info logging
    print("Unsolved (%7d nodes, score %+7.2f)" % (args.max_nodes, last_eval / 100.0),
        flush=True)

def main():
    # Initialize the argument parser
    parser = argparse.ArgumentParser(
        description="A tool for locating endgame holes in an engine's knowledge.",
        epilog="This tool will generate two files when being run:\n"
            + " - samples.epd, which contains a list of all positions analysed;\n"
            + " - failures.epd, which contains a list of all positions that the engine\n"
            + "   failed to evaluate correctly."
    )
    parser.add_argument(
        "--version",
        action="version",
        version="%(prog)s v1.1.0"
    )
    parser.add_argument(
        "engine",
        help="The path of the engine to test",
        metavar="ENGINE"
    )
    parser.add_argument(
        "syzygy_path",
        help="A list of folder paths to search for Syzygy tablebases, separated by '%s'" % PATH_SEPARATOR,
        metavar="SYZYGY_PATH"
    )
    parser.add_argument(
        "--max-nodes",
        default=1000000,
        type=int,
        help="The maximal node count on which the engine will perform searches (default: 1000000)",
        metavar="NODES"
    )
    parser.add_argument(
        "--min-nodes",
        default=10000,
        type=int,
        help="The minimal node count on which the engine will perform searches (default: 10000)",
        metavar="NODES"
    )
    parser.add_argument(
        "--iters",
        default=5,
        type=int,
        help="The number of successive search iterations for filtering away correctly evaluated positions (default: 5, min. 1)",
        metavar="ITERS"
    )
    parser.add_argument(
        "--draw",
        default=100,
        type=int,
        help="The highest absolute value (in cp) for a position to be considered as drawn by the engine (default: 100)",
        metavar="CP"
    )
    parser.add_argument(
        "--win",
        default=200,
        type=int,
        help="The lowest absolute value (in cp) for a position to be considered as won/lost by the engine (default: 200)",
        metavar="CP"
    )
    parser.add_argument(
        "--rate",
        default=20.0,
        type=float,
        help="The rate (in percent) at which positions will be sampled from the PGN file (default: 20.0)",
        metavar="RATE"
    )
    parser.add_argument(
        "--hash",
        default=16,
        type=int,
        help="The amount of hash (in MB) to give to the engine (default: 16)",
        metavar="HASH_MB"
    )
    parser.add_argument(
        "--pgn",
        action="append",
        type=open,
        default=[],
        help="The path of a PGN file to analyse",
        metavar="PGN_FILE",
        dest="pgn_list"
    )
    parser.add_argument(
        "--epd",
        action="append",
        type=open,
        default=[],
        help="The path of an EPD file to analyse",
        metavar="EPD_FILE",
        dest="epd_list"
    )
    parser.add_argument(
        "--quiet",
        action="store_false",
        help="Makes the tool less verbose",
        dest="verbose"
    )

    args = parser.parse_args()

    # Load the Syzygy tablebases
    tb = chess.syzygy.Tablebase()
    for path in args.syzygy_path.split(PATH_SEPARATOR):
        tb.add_directory(path)

    # Initialize the engine
    engine = chess.engine.SimpleEngine.popen_uci(args.engine)

    if "Hash" in engine.options:
        engine.configure({"Hash": args.hash})

    sample_file = open("samples.epd", "w")
    miseval_file = open("failures.epd", "w")

    # Analyse all given EPD files
    count = 0
    for epd_file in args.epd_list:
        for line in epd_file:
            count += 1
            board = chess.Board()
            board.set_epd(line)
            maybe_analyse_board(engine, tb, board, sample_file, miseval_file, "Board %7d" % count, args)

        epd_file.close()

    # Analyse all given PGN files
    count = 0
    for pgn_file in args.pgn_list:
        game = chess.pgn.read_game(pgn_file)

        while game != None:
            count += 1
            board = game.board()

            for move in game.mainline_moves():
                board.push(move)

                # Avoid analysing all adjacent positions since they're most likely similar
                # and will be subject to the same misevals
                if random.uniform(0.0, 100.0) <= args.rate:
                    maybe_analyse_board(engine, tb, board, sample_file, miseval_file, "Game %7d" % count, args)

            game = chess.pgn.read_game(pgn_file)

        pgn_file.close()

    # Final cleanup
    tb.close()
    sample_file.close()
    miseval_file.close()
    engine.quit()

if __name__ == '__main__':
    main()
