from papers_v2.core.models import AnalysisReport, Publication


class AnalysisEngine:
    def __init__(self) -> None:
        pass

    def analyze(self, publication: Publication) -> AnalysisReport:
        raise NotImplementedError
